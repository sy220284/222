//! Source: 2.1.88 src/utils/execFileNoThrow.ts.
//! Provider-neutral, non-throwing argv process execution. External tool/permission
//! approval must happen at the calling boundary, before invoking this helper.
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::watch,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Ignore,
    Inherit,
    Pipe,
}

#[derive(Debug)]
pub struct Options {
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
    pub input_mode: InputMode,
    pub input: Option<Vec<u8>>,
    pub timeout: Duration,
    pub max_buffer: usize,
    pub preserve_output_on_error: bool,
    pub abort: Option<watch::Receiver<bool>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cwd: None,
            env: Vec::new(),
            input_mode: InputMode::Ignore,
            input: None,
            timeout: Duration::from_secs(600),
            max_buffer: 1_000_000,
            preserve_output_on_error: true,
            abort: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
    pub error: Option<String>,
}

impl Output {
    pub fn success(&self) -> bool {
        self.code == 0 && self.error.is_none()
    }
    fn failed(reason: impl Into<String>, preserve: bool) -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            code: 1,
            error: preserve.then(|| reason.into()),
        }
    }
    fn from_exit(
        status: std::process::ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        preserve: bool,
    ) -> Self {
        let code = status.code().unwrap_or(1);
        if status.success() {
            return Self {
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
                code: 0,
                error: None,
            };
        }
        if !preserve {
            return Self::failed("", false).with_code(code);
        }
        Self {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            code,
            error: Some(format!("Command failed with exit code {code}: {status}")),
        }
    }
    fn with_code(mut self, code: i32) -> Self {
        self.code = code;
        self
    }
}

async fn read_bounded(mut stream: impl AsyncRead + Unpin, max: usize) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut block = [0_u8; 8192];
    loop {
        let n = stream.read(&mut block).await?;
        if n == 0 {
            return Ok(out);
        }
        if n > max.saturating_sub(out.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Process output exceeded maxBuffer ({max} bytes)"),
            ));
        }
        out.extend_from_slice(&block[..n]);
    }
}

async fn wait_for_abort(abort: &mut watch::Receiver<bool>) {
    loop {
        if *abort.borrow_and_update() {
            return;
        }
        if abort.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// Run a preconfigured command. Every path, argument and environment override
/// is passed as a separate OS argument (no shell expansion). The caller owns
/// authorization. Spawn failures, nonzero exits, cancellation, buffer overflow
/// and timeouts are all returned as values, not thrown.
pub async fn execute(mut command: Command, mut options: Options) -> Output {
    if let Some(ref abort) = options.abort {
        if *abort.borrow() {
            return Output::failed(
                "Process aborted before spawn",
                options.preserve_output_on_error,
            );
        }
    }
    if let Some(cwd) = options.cwd.take() {
        command.current_dir(cwd);
    }
    for (key, value) in options.env.drain(..) {
        command.env(key, value);
    }
    let should_pipe_stdin = options.input.is_some() || options.input_mode == InputMode::Pipe;
    command.stdin(if should_pipe_stdin {
        Stdio::piped()
    } else if options.input_mode == InputMode::Inherit {
        Stdio::inherit()
    } else {
        Stdio::null()
    });
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::subprocess_env::apply(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Output::failed(
                format!("Could not start process: {error}"),
                options.preserve_output_on_error,
            )
        }
    };
    let stdout = child.stdout.take().expect("stdout was configured as piped");
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let stdin = child.stdin.take();
    let input = options.input.take();
    let max_buffer = options.max_buffer;
    let operation = async {
        let write = async {
            if let Some(mut pipe) = stdin {
                if let Some(data) = input {
                    if let Err(error) = pipe.write_all(&data).await {
                        // Child programs are allowed to close stdin early.
                        if error.kind() != std::io::ErrorKind::BrokenPipe {
                            return Err(error);
                        }
                    }
                }
                drop(pipe); // Deliver EOF to processes reading from stdin.
            }
            Ok::<(), std::io::Error>(())
        };
        let ((), out, err, status) = tokio::try_join!(
            write,
            read_bounded(stdout, max_buffer),
            read_bounded(stderr, max_buffer),
            child.wait()
        )?;
        Ok::<_, std::io::Error>((status, out, err))
    };
    let preserve = options.preserve_output_on_error;
    let result = match options.abort.as_mut() {
        Some(abort) => {
            tokio::select! {
                value = tokio::time::timeout(options.timeout, operation) => Some(value),
                _ = wait_for_abort(abort) => None,
            }
        }
        None => Some(tokio::time::timeout(options.timeout, operation).await),
    };
    match result {
        None => Output::failed("Process aborted", preserve),
        Some(Err(_)) => Output::failed("Process timed out", preserve),
        Some(Ok(Err(error))) => Output::failed(error.to_string(), preserve),
        Some(Ok(Ok((status, stdout, stderr)))) => {
            Output::from_exit(status, stdout, stderr, preserve)
        }
    }
}

/// Direct argv execution equivalent of execFileNoThrowWithCwd (shell=false).
/// cwd, input, environment and cancellation are supplied through Options.
pub async fn exec_file_no_throw(
    file: impl AsRef<OsStr>,
    args: &[&str],
    options: Options,
) -> Output {
    let mut command = Command::new(file);
    command.args(args);
    execute(command, options).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    async fn sh(command: &str, options: Options) -> Output {
        exec_file_no_throw("sh", &["-c", command], options).await
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn argv_is_literal_and_exit_output_is_preserved() {
        let result = exec_file_no_throw(
            "sh",
            &[
                "-c",
                "printf '%s' \"$1\"; printf err >&2; exit 7",
                "sh",
                "$(touch /tmp/NON_EXECUTED_ARG)",
            ],
            Options::default(),
        )
        .await;
        assert_eq!(result.code, 7);
        assert_eq!(result.stdout, "$(touch /tmp/NON_EXECUTED_ARG)");
        assert_eq!(result.stderr, "err");
        assert!(result.error.unwrap().contains('7'));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn failed_process_can_hide_output_and_error_without_losing_exit_code() {
        let options = Options {
            preserve_output_on_error: false,
            ..Options::default()
        };
        let result = sh("printf sensitive; echo error >&2; exit 9", options).await;
        assert_eq!(result.code, 9);
        assert!(result.stdout.is_empty() && result.stderr.is_empty() && result.error.is_none());
    }
    #[tokio::test]
    async fn nonexistent_program_never_throws() {
        let result = exec_file_no_throw(
            "/nonexistent/atlas-command-will-not-exist",
            &[],
            Options::default(),
        )
        .await;
        assert_eq!(result.code, 1);
        assert!(result.error.is_some());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn supplies_cwd_environment_input_and_eof() {
        let root = tempfile::tempdir().unwrap();
        let options = Options {
            cwd: Some(root.path().into()),
            env: vec![("ATLAS_PROCESS_TEST".into(), "value".into())],
            input: Some(b"from input".to_vec()),
            ..Options::default()
        };
        let result = sh(
            "read value; printf '%s|%s|%s' \"$value\" \"$ATLAS_PROCESS_TEST\" \"$(pwd)\"",
            options,
        )
        .await;
        assert_eq!(result.code, 0, "{result:?}");
        assert_eq!(
            result.stdout,
            format!("from input|value|{}", root.path().display())
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_terminates_unresponsive_process() {
        let result = sh(
            "sleep 4",
            Options {
                timeout: Duration::from_millis(40),
                ..Options::default()
            },
        )
        .await;
        assert_eq!(result.code, 1);
        assert!(result.error.unwrap().contains("timed out"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn abort_signal_terminates_running_process() {
        let (send, receive) = watch::channel(false);
        let process = sh(
            "sleep 4",
            Options {
                abort: Some(receive),
                ..Options::default()
            },
        );
        let cancel = async {
            tokio::time::sleep(Duration::from_millis(45)).await;
            send.send(true).unwrap();
        };
        let (result, ()) = tokio::join!(process, cancel);
        assert_eq!(result.code, 1);
        assert!(result.error.unwrap().contains("aborted"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn output_limit_is_enforced_during_read_not_after_allocation() {
        let result = sh(
            "printf '12345678901234567890'",
            Options {
                max_buffer: 8,
                ..Options::default()
            },
        )
        .await;
        assert_eq!(result.code, 1);
        assert!(result.error.unwrap().contains("maxBuffer"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn stdout_stderr_are_drained_concurrently() {
        let result = sh("printf 'out'; printf 'err' >&2", Options::default()).await;
        assert_eq!(result.code, 0);
        assert!(result.success());
        assert_eq!(result.stdout, "out");
        assert_eq!(result.stderr, "err");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn pre_cancelled_command_does_not_execute() {
        let (send, receive) = watch::channel(true);
        let _keep = send;
        let result = sh(
            "printf should-not-run",
            Options {
                abort: Some(receive),
                ..Options::default()
            },
        )
        .await;
        assert!(result.stdout.is_empty());
        assert_eq!(result.code, 1);
    }
}
