//! Cross-platform terminal launcher for external deep links, porting
//! 2.1.88 src/utils/deepLink/terminalLauncher.ts.
//! Passes each query and path as a separate argv argument where possible.
use anyhow::{bail, Context, Result};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInfo {
    pub name: String,
    pub command: PathBuf,
}

pub struct TerminalLaunch {
    pub query: Option<String>,
    pub cwd: Option<PathBuf>,
    pub repo: Option<String>,
    pub last_fetch_ms: Option<u64>,
}

fn which(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if (path.is_absolute() || command.contains('/')) && path.is_file() {
        return Some(path.to_owned());
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let file = dir.join(command);
        if file.is_file() {
            return Some(file);
        }
        #[cfg(windows)]
        for ext in ["exe", "cmd", "bat"] {
            let found = dir.join(format!("{command}.{ext}"));
            if found.is_file() {
                return Some(found);
            }
        }
    }
    None
}

pub fn detect_terminal() -> Option<TerminalInfo> {
    #[cfg(target_os = "linux")]
    {
        if let Some(selected) = std::env::var_os("TERMINAL") {
            let selected = selected.to_string_lossy();
            if let Some(found) = which(&selected) {
                return Some(TerminalInfo {
                    name: Path::new(selected.as_ref())
                        .file_name()?
                        .to_string_lossy()
                        .into_owned(),
                    command: found,
                });
            }
        }
        for name in [
            "x-terminal-emulator",
            "ghostty",
            "kitty",
            "alacritty",
            "wezterm",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "mate-terminal",
            "tilix",
            "xterm",
        ] {
            if let Some(found) = which(name) {
                return Some(TerminalInfo {
                    name: name.into(),
                    command: found,
                });
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        let preferred = std::env::var("TERM_PROGRAM")
            .unwrap_or_default()
            .to_lowercase();
        let name = match preferred.trim_end_matches(".app") {
            "iterm" | "iterm2" => "iTerm",
            "ghostty" => "Ghostty",
            "kitty" => "kitty",
            "alacritty" => "Alacritty",
            "wezterm" => "WezTerm",
            _ => "Terminal",
        };
        Some(TerminalInfo {
            name: name.into(),
            command: PathBuf::from(name),
        })
    }
    #[cfg(target_os = "windows")]
    {
        for (name, command) in [
            ("Windows Terminal", "wt.exe"),
            ("PowerShell", "pwsh.exe"),
            ("PowerShell", "powershell.exe"),
            ("Command Prompt", "cmd.exe"),
        ] {
            if let Some(found) = which(command) {
                return Some(TerminalInfo {
                    name: name.into(),
                    command: found,
                });
            }
        }
        None
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

pub fn program_arguments(action: &TerminalLaunch) -> Vec<OsString> {
    let mut arguments = Vec::new();
    if let Some(cwd) = &action.cwd {
        arguments.push("--workspace".into());
        arguments.push(cwd.as_os_str().to_owned());
    }
    arguments.push("chat".into());
    arguments.push("--deep-link-origin".into());
    if let Some(repo) = &action.repo {
        arguments.push("--deep-link-repo".into());
        arguments.push(repo.into());
    }
    if let Some(when) = action.last_fetch_ms {
        arguments.push("--deep-link-last-fetch".into());
        arguments.push(when.to_string().into());
    }
    if let Some(query) = &action.query {
        arguments.push("--prefill".into());
        arguments.push(query.into());
    }
    arguments
}

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
}

/// Each terminal starts the same current executable. On Linux and macOS
/// modern terminals, no shell interprets user-supplied content.
pub fn build_terminal_plan(
    terminal: &TerminalInfo,
    executable: &Path,
    action: &TerminalLaunch,
) -> Result<LaunchPlan> {
    let arguments = program_arguments(action);
    let mut params = Vec::new();
    let mut cwd = None;
    let name = terminal.name.as_str();
    #[cfg(target_os = "linux")]
    {
        let root = action.cwd.as_deref();
        match name {
            "gnome-terminal" => {
                if let Some(root) = root {
                    params.push(format!("--working-directory={}", root.display()).into());
                }
                params.push("--".into());
            }
            "konsole" => {
                if let Some(root) = root {
                    params.extend([OsString::from("--workdir"), root.as_os_str().to_owned()]);
                }
                params.push("-e".into());
            }
            "kitty" => {
                if let Some(root) = root {
                    params.extend([OsString::from("--directory"), root.as_os_str().to_owned()]);
                }
            }
            "wezterm" => {
                params.push("start".into());
                if let Some(root) = root {
                    params.extend([OsString::from("--cwd"), root.as_os_str().to_owned()]);
                }
                params.push("--".into());
            }
            "alacritty" => {
                if let Some(root) = root {
                    params.extend([
                        OsString::from("--working-directory"),
                        root.as_os_str().to_owned(),
                    ]);
                }
                params.push("-e".into());
            }
            "ghostty" => {
                if let Some(root) = root {
                    params.push(format!("--working-directory={}", root.display()).into());
                }
                params.push("-e".into());
            }
            "xfce4-terminal" | "mate-terminal" => {
                if let Some(root) = root {
                    params.push(format!("--working-directory={}", root.display()).into());
                }
                params.push("-x".into());
            }
            "tilix" => {
                if let Some(root) = root {
                    params.push(format!("--working-directory={}", root.display()).into());
                }
                params.push("-e".into());
            }
            _ => {
                params.push("-e".into());
                cwd = action.cwd.clone();
            }
        }
        params.push(executable.as_os_str().to_owned());
        params.extend(arguments);
    }
    #[cfg(target_os = "macos")]
    {
        if matches!(name, "iTerm" | "Terminal") {
            let mut source = Vec::new();
            if let Some(directory) = &action.cwd {
                source.push("cd".to_owned());
                source.push(posix_quote(&directory.to_string_lossy()));
                source.push("&&".to_owned());
            }
            source.push(posix_quote(&executable.to_string_lossy()));
            source.extend(
                arguments
                    .iter()
                    .map(|arg| posix_quote(&arg.to_string_lossy())),
            );
            let text = source.join(" ");
            let quoted = applescript_quote(&text);
            let script = if name == "iTerm" {
                format!(
                    "tell application \"iTerm\"\n if running then\n create window with default profile\n else\n activate\n end if\n tell current session of current window\n write text {quoted}\n end tell\nend tell"
                )
            } else {
                format!("tell application \"Terminal\"\n do script {quoted}\n activate\nend tell")
            };
            return Ok(LaunchPlan {
                executable: PathBuf::from("/usr/bin/osascript"),
                args: vec!["-e".into(), script.into()],
                cwd: None,
            });
        }
        params.extend([
            "-na".into(),
            terminal.command.as_os_str().to_owned(),
            "--args".into(),
        ]);
        match name {
            "Ghostty" => {
                params.push("--window-save-state=never".into());
                if let Some(root) = &action.cwd {
                    params.push(format!("--working-directory={}", root.display()).into());
                }
                params.push("-e".into());
            }
            "Alacritty" => {
                if let Some(root) = &action.cwd {
                    params.extend([
                        OsString::from("--working-directory"),
                        root.as_os_str().to_owned(),
                    ]);
                }
                params.push("-e".into());
            }
            "kitty" => {
                if let Some(root) = &action.cwd {
                    params.extend([OsString::from("--directory"), root.as_os_str().to_owned()]);
                }
            }
            "WezTerm" => {
                params.push("start".into());
                if let Some(root) = &action.cwd {
                    params.extend([OsString::from("--cwd"), root.as_os_str().to_owned()]);
                }
                params.push("--".into());
            }
            _ => bail!("Unsupported macOS terminal"),
        }
        params.push(executable.as_os_str().to_owned());
        params.extend(arguments);
        return Ok(LaunchPlan {
            executable: PathBuf::from("/usr/bin/open"),
            args: params,
            cwd,
        });
    }
    #[cfg(target_os = "windows")]
    {
        match name {
            "Windows Terminal" => {
                if let Some(root) = &action.cwd {
                    params.extend([OsString::from("-d"), root.as_os_str().to_owned()]);
                }
                params.push("--".into());
                params.push(executable.as_os_str().to_owned());
                params.extend(arguments);
            }
            "PowerShell" => {
                let mut text = String::new();
                if let Some(root) = &action.cwd {
                    text.push_str(&format!(
                        "Set-Location {}; ",
                        powershell_quote(&root.to_string_lossy())
                    ));
                }
                text.push_str(&format!(
                    "& {}",
                    powershell_quote(&executable.to_string_lossy())
                ));
                for item in &arguments {
                    text.push(' ');
                    text.push_str(&powershell_quote(&item.to_string_lossy()));
                }
                params.extend(["-NoExit".into(), "-Command".into(), text.into()]);
            }
            _ => {
                let mut text = String::new();
                if let Some(root) = &action.cwd {
                    text.push_str(&format!("cd /d {} && ", cmd_quote(&root.to_string_lossy())));
                }
                text.push_str(&cmd_quote(&executable.to_string_lossy()));
                for item in &arguments {
                    text.push(' ');
                    text.push_str(&cmd_quote(&item.to_string_lossy()));
                }
                params.extend(["/k".into(), text.into()]);
            }
        }
    }
    Ok(LaunchPlan {
        executable: terminal.command.clone(),
        args: params,
        cwd,
    })
}

#[cfg(target_os = "macos")]
fn posix_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn applescript_quote(arg: &str) -> String {
    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "windows")]
fn powershell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn cmd_quote(arg: &str) -> String {
    // cmd.exe does not treat backslash-quote as an escaped quote. An embedded
    // double quote would end the quoted region and expose &, |, < or > as
    // commands. Match 2.1.88 terminalLauncher.ts: remove embedded quotes,
    // escape percent expansion and double trailing backslashes for the
    // child's Windows argv parser.
    let mut escaped = arg.replace('"', "").replace('%', "%%");
    let trailing = escaped
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count();
    escaped.push_str(&"\\".repeat(trailing));
    format!("\"{escaped}\"")
}

/// macOS launches via short-lived osascript/open helpers. The original
/// terminalLauncher.ts waits for their exit status and falls back to
/// Terminal.app when a preferred emulator cannot be opened.
#[cfg(target_os = "macos")]
fn run_macos_plan(plan: &LaunchPlan) -> Result<bool> {
    let status = Command::new(&plan.executable)
        .args(&plan.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Cannot launch {}", plan.executable.display()))?;
    Ok(status.success())
}

#[cfg(target_os = "macos")]
pub fn launch_in_terminal(executable: &Path, action: &TerminalLaunch) -> Result<()> {
    let selected = detect_terminal().context("No supported terminal emulator installed")?;
    let primary = build_terminal_plan(&selected, executable, action)?;
    if matches!(run_macos_plan(&primary), Ok(true)) {
        return Ok(());
    }
    if selected.name != "Terminal" {
        let fallback = TerminalInfo {
            name: "Terminal".into(),
            command: PathBuf::from("Terminal"),
        };
        let plan = build_terminal_plan(&fallback, executable, action)?;
        if run_macos_plan(&plan)? {
            return Ok(());
        }
    }
    bail!("Failed to open a supported macOS terminal emulator")
}

#[cfg(not(target_os = "macos"))]
pub fn launch_in_terminal(executable: &Path, action: &TerminalLaunch) -> Result<()> {
    let selected = detect_terminal().context("No supported terminal emulator installed")?;
    let plan = build_terminal_plan(&selected, executable, action)?;
    let mut command = Command::new(&plan.executable);
    // cmd.exe parses /k as a shell command, not an MSVCRT argv string.
    // Rust's normal Windows argument quoting would double-escape the
    // already cmd-quoted command; pass that one argument verbatim.
    #[cfg(target_os = "windows")]
    {
        if selected.name == "Command Prompt" {
            use std::os::windows::process::CommandExt;
            let mut args = plan.args.into_iter();
            if let Some(switch) = args.next() {
                command.arg(switch);
            }
            for raw in args {
                command.raw_arg(raw);
            }
        } else {
            command.args(plan.args);
        }
    }
    #[cfg(not(target_os = "windows"))]
    command.args(plan.args);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(root) = plan.cwd {
        command.current_dir(root);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command
        .spawn()
        .context("Unable to launch terminal emulator")?;
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn correct_separate_argv_preserves_unicode_quotes_and_shell_symbols() {
        let action = TerminalLaunch {
            query: Some("修复 'a'; $(rm -rf /) && review".into()),
            cwd: Some(PathBuf::from("/tmp/project folder")),
            repo: Some("org/repo".into()),
            last_fetch_ms: Some(123456789),
        };
        let program = PathBuf::from("/usr/bin/atlas-agent");
        let terminal = TerminalInfo {
            name: "kitty".into(),
            command: "/usr/bin/kitty".into(),
        };
        let plan = build_terminal_plan(&terminal, &program, &action).unwrap();
        assert_eq!(plan.args[0], "--directory");
        assert_eq!(plan.args[1], "/tmp/project folder");
        assert!(plan
            .args
            .contains(&OsString::from("修复 'a'; $(rm -rf /) && review")));
        assert!(plan.args.contains(&OsString::from("org/repo")));
        assert!(!plan.args.contains(&OsString::from("sh")));
        assert!(plan.args.contains(&OsString::from("--prefill")));
    }

    #[test]
    fn terminal_config_uses_native_cwd_flags_or_spawn_directory() {
        let action = TerminalLaunch {
            cwd: Some(PathBuf::from("/tmp/home with spaces")),
            query: None,
            repo: None,
            last_fetch_ms: None,
        };
        let bin = Path::new("/bin/atlas");
        let gnome = build_terminal_plan(
            &TerminalInfo {
                name: "gnome-terminal".into(),
                command: "gnome-terminal".into(),
            },
            bin,
            &action,
        )
        .unwrap();
        assert_eq!(gnome.args[0], "--working-directory=/tmp/home with spaces");
        let xterm = build_terminal_plan(
            &TerminalInfo {
                name: "xterm".into(),
                command: "xterm".into(),
            },
            bin,
            &action,
        )
        .unwrap();
        assert_eq!(xterm.cwd, action.cwd);
    }
}
