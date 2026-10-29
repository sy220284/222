//! Synchronous shell execution compatibility for 2.1.88
//! src/utils/execFileNoThrowPortable.ts and src/utils/execSyncWrapper.ts.
//! The caller must authorize shell commands before entering this module.
//! This deliberately blocks its calling thread, matching the original
//! deprecated API. Prefer src/exec_file.rs from async call sites.
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncStdio {
    Ignore,
    #[default]
    Pipe,
    Inherit,
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub input: Option<Vec<u8>>,
    pub stdin: SyncStdio,
    pub stdout: SyncStdio,
    pub stderr: SyncStdio,
    pub env: Vec<(OsString, OsString)>,
    pub max_buffer: usize,
    pub abort: Option<Arc<AtomicBool>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            cwd: None,
            timeout: Duration::from_secs(600),
            input: None,
            stdin: SyncStdio::Ignore,
            stdout: SyncStdio::Pipe,
            stderr: SyncStdio::Pipe,
            env: Vec::new(),
            max_buffer: 1_000_000,
            abort: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

fn stdio(mode: SyncStdio) -> Stdio {
    match mode {
        SyncStdio::Ignore => Stdio::null(),
        SyncStdio::Pipe => Stdio::piped(),
        SyncStdio::Inherit => Stdio::inherit(),
    }
}

fn bounded_reader<R: Read + Send + 'static>(
    mut reader: R,
    cap: usize,
    overflow: Arc<AtomicBool>,
) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buf = [0u8; 8192];
        let result = loop {
            match reader.read(&mut buf) {
                Ok(0) => break Ok(bytes),
                Ok(n) if n > cap.saturating_sub(bytes.len()) => {
                    overflow.store(true, Ordering::Release);
                    break Err(std::io::Error::other(format!(
                        "Process output exceeded maxBuffer ({cap} bytes)"
                    )));
                }
                Ok(n) => bytes.extend_from_slice(&buf[..n]),
                Err(error) => break Err(error),
            }
        };
        let _ = tx.send(result);
    });
    rx
}

fn read_result(
    rx: Option<mpsc::Receiver<std::io::Result<Vec<u8>>>>,
    started: Instant,
    timeout: Duration,
    abort: Option<&Arc<AtomicBool>>,
) -> anyhow::Result<Vec<u8>> {
    use anyhow::{bail, Context};
    let Some(rx) = rx else {
        return Ok(Vec::new());
    };
    // A descendant may keep stdout/stderr open after its parent exits.
    // Continue waiting while the caller's original timeout permits it;
    // a fixed post-exit deadline discards valid, slow pipe output.
    loop {
        if abort.is_some_and(|a| a.load(Ordering::Acquire)) {
            bail!("Process aborted while draining output");
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            // Accept a result that arrived exactly as the deadline elapsed.
            return rx
                .try_recv()
                .context("Process output timed out")?
                .map_err(Into::into);
        }
        match rx.recv_timeout(remaining.min(Duration::from_millis(30))) {
            Ok(result) => return result.map_err(Into::into),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                bail!("Process output reader disconnected before completion");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
        }
    }
}

/// Shell semantics are intentional: the input is a complete user-authorized
/// shell command. Windows uses cmd /C; POSIX uses sh -c.
/// A non-zero process exit is represented in the returned output, while
/// spawn, timeout, cancellation and stream errors use Err.
pub fn run_shell(command: &str, options: SyncOptions) -> anyhow::Result<SyncOutput> {
    use anyhow::{bail, Context};
    if command.trim().is_empty() {
        bail!("Empty process command");
    }
    if options
        .abort
        .as_ref()
        .is_some_and(|a| a.load(Ordering::Acquire))
    {
        bail!("Process aborted before spawn");
    }
    #[cfg(windows)]
    let mut process = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    };
    #[cfg(not(windows))]
    let mut process = {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    };
    if let Some(cwd) = options.cwd {
        process.current_dir(cwd);
    }
    for (k, v) in options.env {
        process.env(k, v);
    }
    process.stdin(if options.input.is_some() {
        Stdio::piped()
    } else {
        stdio(options.stdin)
    });
    process
        .stdout(stdio(options.stdout))
        .stderr(stdio(options.stderr));
    let mut child = process.spawn().context("Cannot run local shell command")?;
    let overflow = Arc::new(AtomicBool::new(false));
    let out = child
        .stdout
        .take()
        .map(|s| bounded_reader(s, options.max_buffer, overflow.clone()));
    let err = child
        .stderr
        .take()
        .map(|s| bounded_reader(s, options.max_buffer, overflow.clone()));
    if let Some(mut stdin) = child.stdin.take() {
        // Input is supplied on an independent worker to avoid deadlocks when
        // the child writes output before reading stdin.
        std::thread::spawn(move || {
            if let Some(input) = options.input {
                let _ = stdin.write_all(&input);
            }
            // Dropping stdin signals EOF, including after a broken pipe.
        });
    }
    let started = Instant::now();
    let result = loop {
        if options
            .abort
            .as_ref()
            .is_some_and(|a| a.load(Ordering::Acquire))
        {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Process aborted");
        }
        if overflow.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "Process output exceeded maxBuffer ({} bytes)",
                options.max_buffer
            );
        }
        if started.elapsed() >= options.timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Process timed out after {} ms", options.timeout.as_millis());
        }
        if let Some(status) = child
            .try_wait()
            .context("Cannot wait for local shell command")?
        {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = read_result(out, started, options.timeout, options.abort.as_ref())?;
    let stderr = read_result(err, started, options.timeout, options.abort.as_ref())?;
    Ok(SyncOutput {
        code: result.code().unwrap_or(1),
        stdout,
        stderr,
    })
}

/// Source-compatible deprecated helper: nonzero exit codes still produce
/// stdout, and empty/whitespace stdout or spawn/timeout errors yield None.
pub fn exec_sync_with_defaults_deprecated(command: &str, options: SyncOptions) -> Option<String> {
    run_shell(command, options).ok().and_then(|output| {
        let text = String::from_utf8_lossy(&output.stdout);
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    })
}

/// Strict wrapper corresponding to node child_process.execSync. Nonzero
/// exits raise an error instead of silently returning stdout.
pub fn exec_sync_deprecated(command: &str, options: SyncOptions) -> anyhow::Result<Vec<u8>> {
    use anyhow::bail;
    let output = run_shell(command, options)?;
    if output.code != 0 {
        bail!(
            "Command failed with exit code {}: {}",
            output.code,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn forwards_shell_and_nonzero_output_without_throwing() {
        let out = run_shell(
            "printf 'okay'; echo 'error' >&2; exit 12",
            SyncOptions::default(),
        )
        .unwrap();
        assert_eq!(out.code, 12);
        assert_eq!(out.stdout, b"okay");
        assert_eq!(out.stderr, b"error\n");
        assert_eq!(
            exec_sync_with_defaults_deprecated(
                "printf ' okay\\n '; exit 9",
                SyncOptions::default()
            )
            .as_deref(),
            Some("okay")
        );
        assert!(exec_sync_deprecated("exit 3", SyncOptions::default()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn stdin_env_and_working_directory_are_effective() {
        let d = tempfile::tempdir().unwrap();
        let opts = SyncOptions {
            cwd: Some(d.path().to_path_buf()),
            input: Some(b"hello\n".to_vec()),
            env: vec![("ATLAS_SYNC_TEST".into(), "world".into())],
            ..Default::default()
        };
        let output = run_shell(
            "read word; printf '%s|%s|%s' \"$word\" \"$ATLAS_SYNC_TEST\" \"$(pwd)\"",
            opts,
        )
        .unwrap();
        assert_eq!(
            output.stdout,
            format!("hello|world|{}", d.path().display()).as_bytes()
        );
    }
    #[cfg(unix)]
    #[test]
    fn timeout_kills_process_and_returns_no_fallback_output() {
        let start = Instant::now();
        assert_eq!(
            exec_sync_with_defaults_deprecated(
                "sleep 5",
                SyncOptions {
                    timeout: Duration::from_millis(40),
                    ..Default::default()
                }
            ),
            None
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[cfg(unix)]
    #[test]
    fn abort_cancels_running_process_and_pre_abort_skips_spawn() {
        let abort = Arc::new(AtomicBool::new(false));
        let other = abort.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(25));
            other.store(true, Ordering::Release);
        });
        assert!(run_shell(
            "sleep 5",
            SyncOptions {
                abort: Some(abort),
                ..Default::default()
            }
        )
        .is_err());
        stopper.join().unwrap();
        let pre = Arc::new(AtomicBool::new(true));
        assert!(run_shell(
            "echo nope",
            SyncOptions {
                abort: Some(pre),
                ..Default::default()
            }
        )
        .is_err());
    }
    #[cfg(unix)]
    #[test]
    fn output_overflow_is_rejected_while_process_is_alive() {
        assert!(run_shell(
            "printf '1234567890'; sleep 4",
            SyncOptions {
                max_buffer: 3,
                ..Default::default()
            }
        )
        .is_err());
    }
    #[cfg(unix)]
    #[test]
    fn output_from_inherited_pipe_after_shell_exit_respects_global_deadline() {
        // A child may exit while a descendant legitimately holds stdout open.
        // The original wrapper awaits the pipe; a fixed 250 ms cutoff loses it.
        let output = run_shell(
            "sh -c 'sleep 0.38; printf delayed' &",
            SyncOptions {
                timeout: Duration::from_secs(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output.code, 0);
        assert_eq!(output.stdout, b"delayed");
    }

    #[cfg(unix)]
    #[test]
    fn inherited_pipe_still_honors_global_timeout() {
        let started = Instant::now();
        let error = run_shell(
            "sh -c 'sleep 0.35' &",
            SyncOptions {
                timeout: Duration::from_millis(80),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn inherited_pipe_wait_can_be_cancelled_after_parent_exit() {
        let flag = Arc::new(AtomicBool::new(false));
        let signal = flag.clone();
        let cancel = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(75));
            signal.store(true, Ordering::Release);
        });
        let result = run_shell(
            "sh -c 'sleep 0.35' &",
            SyncOptions {
                abort: Some(flag),
                timeout: Duration::from_secs(1),
                ..Default::default()
            },
        );
        cancel.join().unwrap();
        assert!(result.unwrap_err().to_string().contains("aborted"));
    }

    #[cfg(unix)]
    #[test]
    fn stdio_options_and_empty_stdout_are_honored() {
        let o = SyncOptions {
            stdout: SyncStdio::Ignore,
            stderr: SyncStdio::Ignore,
            ..Default::default()
        };
        assert_eq!(exec_sync_with_defaults_deprecated("echo hello", o), None);
        assert_eq!(
            exec_sync_with_defaults_deprecated("printf '  '", SyncOptions::default()),
            None
        );
    }
}
