//! Provider-neutral shell execution path, following the 2.1.88 shell type,
//! default-shell resolution and PowerShell detection rules.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
};
use tokio::process::Command;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    #[default]
    Bash,
    Powershell,
}

/// Source: BashTool/commandSemantics.ts::interpretCommandResult.
/// Exit code 1 is an informational result for search, comparison and test
/// commands, while other tools continue to treat it as a failure.
pub fn interpret_bash_exit(command: &str, exit_code: i32) -> (bool, Option<&'static str>) {
    use crate::shell_quote::ParseEntry;
    let mut base = String::new();
    if let Ok(entries) = crate::shell_quote::parse(command) {
        let mut expecting_command = true;
        for entry in entries {
            match entry {
                ParseEntry::Operator(op)
                    if matches!(op.as_str(), "|" | "||" | "&&" | ";" | "&") =>
                {
                    expecting_command = true;
                }
                ParseEntry::Word(word) if expecting_command => {
                    base = word;
                    expecting_command = false;
                }
                _ => {}
            }
        }
    }
    if base.is_empty() {
        base = command.split_whitespace().next().unwrap_or("").to_owned();
    }
    match base.as_str() {
        "grep" | "rg" => (
            exit_code >= 2 || exit_code < 0,
            (exit_code == 1).then_some("No matches found"),
        ),
        "find" => (
            exit_code >= 2 || exit_code < 0,
            (exit_code == 1).then_some("Some directories were inaccessible"),
        ),
        "diff" => (
            exit_code >= 2 || exit_code < 0,
            (exit_code == 1).then_some("Files differ"),
        ),
        "test" | "[" => (
            exit_code >= 2 || exit_code < 0,
            (exit_code == 1).then_some("Condition is false"),
        ),
        _ => (exit_code != 0, None),
    }
}

impl ShellKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Powershell => "powershell",
        }
    }

    pub fn build_command(self, command: &str) -> Result<Command> {
        if command.trim().is_empty() {
            bail!("Shell command cannot be empty");
        }
        match self {
            Self::Bash => {
                #[cfg(windows)]
                let path = if let Some(configured) = env::var_os("ATLAS_GIT_BASH_PATH") {
                    // The source refuses an invalid explicit Git Bash path.
                    // Silently falling back would execute a different shell.
                    let configured = PathBuf::from(configured);
                    if !executable(&configured) {
                        bail!("Configured ATLAS_GIT_BASH_PATH is not an executable file");
                    }
                    configured
                } else {
                    crate::portable_platform::find_git_bash_path(None)
                        .context("Git Bash is unavailable; install Git for Windows or configure ATLAS_GIT_BASH_PATH")?
                };
                #[cfg(not(windows))]
                let path = which("bash").context("Bash executable is not available in PATH")?;
                let mut cmd = Command::new(path);
                cmd.args(["-c", command]);
                Ok(cmd)
            }
            Self::Powershell => {
                let path = find_powershell()
                    .context("PowerShell is not installed or not available in PATH")?;
                let mut cmd = Command::new(path);
                cmd.args(build_powershell_args(command));
                Ok(cmd)
            }
        }
    }
}

#[cfg(windows)]
mod windows_job {
    use anyhow::{bail, Context, Result};
    use std::{ffi::c_void, mem, ptr};
    type Handle = *mut c_void;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_SET_QUOTA: u32 = 0x0100;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

    #[repr(C)]
    struct BasicLimits {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }
    #[repr(C)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }
    #[repr(C)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(security: *const c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, kind: u32, data: *const c_void, size: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    pub struct ProcessJob(Handle);
    // Win32 Job handles are kernel-owned and may be used across threads.
    unsafe impl Send for ProcessJob {}
    unsafe impl Sync for ProcessJob {}
    impl Drop for ProcessJob {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    impl ProcessJob {
        pub fn attach(pid: Option<u32>) -> Result<Self> {
            let pid = pid.context("Windows shell process ID missing")?;
            let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error()).context("Create Windows process job");
            }
            let job = Self(handle);
            let mut limits: ExtendedLimits = unsafe { mem::zeroed() };
            limits.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    job.0,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &limits as *const _ as *const c_void,
                    mem::size_of::<ExtendedLimits>() as u32,
                )
            };
            if configured == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("Enable kill-on-close for Windows background process job");
            }
            let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid) };
            if process.is_null() {
                return Err(std::io::Error::last_os_error())
                    .context("Open Windows child for process-job assignment");
            }
            let assigned = unsafe { AssignProcessToJobObject(job.0, process) };
            unsafe {
                CloseHandle(process);
            }
            if assigned == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("Contain Windows shell descendants in job");
            }
            Ok(job)
        }
        pub fn terminate(&self) -> Result<()> {
            if unsafe { TerminateJobObject(self.0, 137) } == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("Terminate Windows background process job");
            }
            Ok(())
        }
    }
}
#[cfg(windows)]
pub use windows_job::ProcessJob as WindowsProcessJob;

/// Original ShellCommand.ts::doKill uses tree-kill rather than killing only
/// the shell. On Windows, Child::kill terminates the parent process but leaves
/// child processes running (and often holding stdout/stderr open). Use the
/// Windows system taskkill executable with /T to terminate the entire tree.
#[cfg(windows)]
pub async fn terminate_windows_process_tree(pid: Option<u32>) -> Result<()> {
    use std::process::Stdio;
    use std::time::Duration;
    let pid = pid.context("Windows shell process ID is unavailable")?;
    let system_root = env::var_os("SystemRoot")
        .or_else(|| env::var_os("WINDIR"))
        .context("Windows system directory is unavailable for process-tree termination")?;
    let taskkill = PathBuf::from(system_root)
        .join("System32")
        .join("taskkill.exe");
    let mut command = Command::new(taskkill);
    command
        .args(["/F", "/T", "/PID"])
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let result = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .context("Windows process-tree termination timed out")?
        .context("Cannot start Windows process-tree terminator")?;
    if !result.status.success() {
        bail!(
            "Windows process-tree termination failed for PID {pid}: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(())
}

/// Resolve a program without invoking a shell or interpreting its contents.
pub fn which(program: &str) -> Option<PathBuf> {
    which_in(program, env::var_os("PATH").as_deref())
}
fn which_in(program: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    if program.trim().is_empty() {
        return None;
    }
    let name = Path::new(program);
    if name.components().count() > 1 || name.is_absolute() {
        return executable(name).then(|| name.to_path_buf());
    }
    let path = path?;
    for dir in env::split_paths(path) {
        let candidate = dir.join(program);
        if executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let suffixes = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
            for ext in suffixes.split(';') {
                let with_ext = dir.join(format!("{program}{ext}"));
                if executable(&with_ext) {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}
fn executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn snap_launcher(path: &Path) -> bool {
    path.to_string_lossy().starts_with("/snap/")
        || path
            .canonicalize()
            .is_ok_and(|p| p.to_string_lossy().starts_with("/snap/"))
}
/// Prefers PowerShell Core and skips snap wrappers in favor of native installs
/// when a native binary is available.
pub fn find_powershell() -> Option<PathBuf> {
    if let Some(path) = which("pwsh") {
        if cfg!(target_os = "linux") && snap_launcher(&path) {
            for candidate in ["/opt/microsoft/powershell/7/pwsh", "/usr/bin/pwsh"] {
                let native = PathBuf::from(candidate);
                if executable(&native) && !snap_launcher(&native) {
                    return Some(native);
                }
            }
        }
        return Some(path);
    }
    which("powershell")
}

pub fn powershell_edition(path: &Path) -> &'static str {
    let base = path.to_string_lossy();
    let base = base.rsplit(['/', '\\']).next().unwrap_or("");
    let base = base.to_ascii_lowercase();
    if base == "pwsh" || base == "pwsh.exe" {
        "core"
    } else {
        "desktop"
    }
}

pub fn build_powershell_args(command: &str) -> [&str; 4] {
    ["-NoProfile", "-NonInteractive", "-Command", command]
}
// src/utils/shell/outputLimits.ts — preserve exported upper/default limits.
pub const BASH_MAX_OUTPUT_UPPER_LIMIT: usize = 150_000;
pub const BASH_MAX_OUTPUT_DEFAULT: usize = 30_000;

pub fn max_output_length_for_value(value: Option<&str>) -> usize {
    crate::env_validation::validate(
        "BASH_MAX_OUTPUT_LENGTH",
        value,
        BASH_MAX_OUTPUT_DEFAULT as u64,
        BASH_MAX_OUTPUT_UPPER_LIMIT as u64,
    )
    .effective as usize
}

pub fn max_output_length() -> usize {
    max_output_length_for_value(std::env::var("BASH_MAX_OUTPUT_LENGTH").ok().as_deref())
}
pub fn from_config(value: Option<&str>) -> Result<ShellKind> {
    match value.unwrap_or("bash") {
        "bash" => Ok(ShellKind::Bash),
        "powershell" => Ok(ShellKind::Powershell),
        other => bail!("Unsupported default shell: {other}; expected bash or powershell"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bash_output_limits_match_source_get_max_output_length() {
        assert_eq!(BASH_MAX_OUTPUT_UPPER_LIMIT, 150_000);
        assert_eq!(BASH_MAX_OUTPUT_DEFAULT, 30_000);
        let scenarios = [
            (None, 30_000),
            (Some(""), 30_000),
            (Some("0"), 30_000),
            (Some("-2"), 30_000),
            (Some("invalid"), 30_000),
            (Some("12.9"), 12),
            (Some("1e3"), 1),
            (Some("+42 trailing"), 42),
            (Some("150000"), 150_000),
            (Some("150001"), 150_000),
            (Some("99999999999999999999999999"), 150_000),
        ];
        for (raw, expected) in scenarios {
            assert_eq!(max_output_length_for_value(raw), expected, "{raw:?}");
        }
    }

    #[test]
    fn default_and_provider_protocol_match() {
        assert_eq!(from_config(None).unwrap(), ShellKind::Bash);
        assert_eq!(
            from_config(Some("powershell")).unwrap(),
            ShellKind::Powershell
        );
        assert!(from_config(Some("cmd")).is_err());
        assert_eq!(
            build_powershell_args("Write-Output 'hello'"),
            [
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Write-Output 'hello'"
            ]
        );
        assert_eq!(powershell_edition(Path::new(r"C:\Tools\pwsh.exe")), "core");
        assert_eq!(powershell_edition(Path::new("/usr/bin/pwsh")), "core");
        assert_eq!(powershell_edition(Path::new("powershell.exe")), "desktop");
    }
    #[test]
    fn lookup_only_returns_actual_executables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("program");
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(which_in("program", Some(dir.path().as_os_str())).is_none());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(
            which_in("program", Some(dir.path().as_os_str())),
            Some(path)
        );
        assert_eq!(which_in("", Some(dir.path().as_os_str())), None);
        assert_eq!(which_in("absent", Some(dir.path().as_os_str())), None);
    }
    #[tokio::test]
    async fn shell_process_propagates_output_and_exit_status() {
        if which("bash").is_none() {
            return;
        }
        let output = ShellKind::Bash
            .build_command("printf 'test'; exit 7")
            .unwrap()
            .output()
            .await
            .unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "test");
        assert_eq!(output.status.code(), Some(7));
    }
}
