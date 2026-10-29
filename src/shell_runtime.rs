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
                let path =
                    which("bash").context("Bash requires Git for Windows or bash in PATH")?;
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
pub fn max_output_length() -> usize {
    crate::env_validation::validate(
        "BASH_MAX_OUTPUT_LENGTH",
        std::env::var("BASH_MAX_OUTPUT_LENGTH").ok().as_deref(),
        30_000,
        150_000,
    )
    .effective as usize
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
