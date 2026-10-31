//! Original 2.1.88 utils/suggestions/shellHistoryCompletion.ts:
//! complete the current Bash input against the 50 most recent distinct
//! !-prefixed commands, preserving spaces and case exactly.
//!
//! The CLI reloads prompt history before every input, so the current turn
//! already holds the up-to-date snapshot. No separate TTL cache is needed.
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellHistoryMatch<'a> {
    pub full_command: &'a str,
    pub suffix: &'a str,
}

/// `input` excludes the leading Bash `!`. History includes it.
pub fn get_shell_history_completion<'a>(
    input: &str,
    history: &'a [String],
) -> Option<ShellHistoryMatch<'a>> {
    if input.len() < 2 || input.trim().is_empty() {
        return None;
    }
    let mut seen = HashSet::new();
    for entry in history {
        let Some(command) = entry.strip_prefix('!') else {
            continue;
        };
        let command = command.trim();
        if command.is_empty() || !seen.insert(command) {
            continue;
        }
        if command.starts_with(input) && command != input {
            return Some(ShellHistoryMatch {
                full_command: command,
                suffix: &command[input.len()..],
            });
        }
        if seen.len() >= 50 {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_recent_distinct_bash_history_exactly() {
        let history = vec![
            "hello".into(),
            "!git status".into(),
            "!git status".into(),
            "!git stash".into(),
            "!ls -lah".into(),
        ];
        assert_eq!(
            get_shell_history_completion("git st", &history),
            Some(ShellHistoryMatch {
                full_command: "git status",
                suffix: "atus",
            })
        );
        assert_eq!(get_shell_history_completion("git  st", &history), None);
        assert_eq!(get_shell_history_completion("Git st", &history), None);
        assert_eq!(
            get_shell_history_completion("ls ", &history).unwrap().suffix,
            "-lah"
        );
        assert_eq!(get_shell_history_completion("ls  ", &history), None);
        assert_eq!(get_shell_history_completion("g", &history), None);
        assert_eq!(get_shell_history_completion("  ", &history), None);
        assert_eq!(get_shell_history_completion("git status", &history), None);
    }

    #[test]
    fn ignores_commands_after_fifty_distinct_entries() {
        let mut history = (0..50).map(|n| format!("!other-{n}")).collect::<Vec<_>>();
        history.push("!cargo test".into());
        assert!(get_shell_history_completion("cargo ", &history).is_none());
        history.insert(0, "!cargo test".into());
        assert_eq!(
            get_shell_history_completion("cargo ", &history).unwrap().suffix,
            "test"
        );
    }

    #[test]
    fn unicode_input_indexes_on_char_boundary() {
        let history = vec!["!查询 状态".to_owned()];
        assert_eq!(
            get_shell_history_completion("查询 ", &history).unwrap().suffix,
            "状态"
        );
    }
}
