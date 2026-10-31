//! Port of utils/renderOptions.ts: stdin override for piped interactive TTY.
//! SDK and CI must keep owning their structured stdin; there is one cached
//! controlling terminal handle per process.
#[cfg(unix)]
use std::{
    fs::File,
    io::{self, IsTerminal},
    sync::OnceLock,
};

/// Environment and argv guards are exact: the source excludes CI, MCP,
/// Windows, or an already-attached stdin terminal.
pub fn should_use_tty_override(
    stdin_is_tty: bool,
    ci_enabled: bool,
    mcp_argv: bool,
    windows: bool,
) -> bool {
    !stdin_is_tty && !ci_enabled && !mcp_argv && !windows
}

#[cfg(unix)]
static TTY_INPUT: OnceLock<Option<File>> = OnceLock::new();

#[cfg(unix)]
pub fn stdin_override() -> Option<File> {
    let stdin_is_tty = io::stdin().is_terminal();
    let ci = crate::env_runtime::is_env_truthy(std::env::var("CI").ok().as_deref());
    let mcp = std::env::args_os().any(|arg| arg == "mcp");
    if !should_use_tty_override(stdin_is_tty, ci, mcp, false) {
        return None;
    }
    let handle = TTY_INPUT.get_or_init(|| match File::open("/dev/tty") {
        Ok(file) => Some(file),
        Err(error) => {
            eprintln!("Controlling terminal unavailable for piped interactive input: {error}");
            None
        }
    });
    match handle {
        Some(file) => match file.try_clone() {
            Ok(input) => Some(input),
            Err(error) => {
                eprintln!("Cannot clone cached terminal input: {error}");
                None
            }
        },
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_tty_override_guards_preserve_machine_input() {
        assert!(should_use_tty_override(false, false, false, false));
        assert!(!should_use_tty_override(true, false, false, false));
        assert!(!should_use_tty_override(false, true, false, false));
        assert!(!should_use_tty_override(false, false, true, false));
        assert!(!should_use_tty_override(false, false, false, true));
    }
}
