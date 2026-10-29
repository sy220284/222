//! Faithful portable subset of BashTool/bashPermissions.ts and
//! PowerShellTool/powershellPermissions.ts permission *suggestion* behavior.
//! This does not implement complete shell AST permissions or UI approval.
use crate::permission_rules::{suggestion_for_exact_command, suggestion_for_prefix};
use serde_json::{json, Value};

const SAFE_ENV_VARS: &[&str] = &[
    "GOEXPERIMENT",
    "GOOS",
    "GOARCH",
    "CGO_ENABLED",
    "GO111MODULE",
    "RUST_BACKTRACE",
    "RUST_LOG",
    "NODE_ENV",
    "PYTHONUNBUFFERED",
    "PYTHONDONTWRITEBYTECODE",
    "PYTEST_DISABLE_PLUGIN_AUTOLOAD",
    "PYTEST_DEBUG",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_TIME",
    "CHARSET",
    "TERM",
    "COLORTERM",
    "NO_COLOR",
    "FORCE_COLOR",
    "TZ",
    "LS_COLORS",
    "LSCOLORS",
    "GREP_COLOR",
    "GREP_COLORS",
    "GCC_COLORS",
    "TIME_STYLE",
    "BLOCK_SIZE",
    "BLOCKSIZE",
];
const BARE_SHELL_PREFIXES: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "csh",
    "tcsh",
    "ksh",
    "dash",
    "cmd",
    "powershell",
    "pwsh",
    "env",
    "xargs",
    "nice",
    "stdbuf",
    "nohup",
    "timeout",
    "time",
    "sudo",
    "doas",
    "pkexec",
];
fn assignment_name(token: &str) -> Option<&str> {
    let (left, _) = token.split_once('=')?;
    let mut bytes = left.bytes();
    let first = bytes.next()?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    if !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    Some(left)
}
fn stripped_safe_env_count(tokens: &[&str]) -> Option<usize> {
    let mut i = 0;
    while i < tokens.len() {
        let Some(name) = assignment_name(tokens[i]) else {
            break;
        };
        // The original internal-only env names are intentionally excluded:
        // they can change remote endpoints or effective execution context.
        if !SAFE_ENV_VARS.contains(&name) {
            return None;
        }
        i += 1;
    }
    Some(i)
}
fn is_subcommand(s: &str) -> bool {
    let mut fragments = s.split('-');
    let first = fragments.next().unwrap_or("");
    if first.is_empty()
        || !first.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        || !first
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return false;
    }
    fragments.all(|fragment| {
        !fragment.is_empty()
            && fragment
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}
pub fn get_simple_command_prefix(command: &str) -> Option<String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let i = stripped_safe_env_count(&tokens)?;
    if tokens.len() < i + 2 || !is_subcommand(tokens[i + 1]) {
        return None;
    }
    Some(format!("{} {}", tokens[i], tokens[i + 1]))
}
pub fn get_first_word_prefix(command: &str) -> Option<String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let i = stripped_safe_env_count(&tokens)?;
    let name = *tokens.get(i)?;
    if !is_subcommand(name) || BARE_SHELL_PREFIXES.contains(&name) {
        return None;
    }
    Some(name.to_owned())
}
pub fn extract_prefix_before_heredoc(command: &str) -> Option<String> {
    let index = command.find("<<")?;
    if index == 0 {
        return None;
    }
    let before = command[..index].trim();
    if before.is_empty() {
        return None;
    }
    if let Some(prefix) = get_simple_command_prefix(before) {
        return Some(prefix);
    }
    let tokens = before.split_whitespace().collect::<Vec<_>>();
    let i = stripped_safe_env_count(&tokens)?;
    if i >= tokens.len() {
        return None;
    }
    let end = (i + 2).min(tokens.len());
    Some(tokens[i..end].join(" "))
}
/// BashTool/bashPermissions.ts::suggestionForExactCommand, not just the
/// shared shellRuleMatching.ts pure helper.
pub fn bash_suggestions(command: &str) -> Value {
    if let Some(prefix) = extract_prefix_before_heredoc(command) {
        return suggestion_for_prefix("Bash", &prefix);
    }
    if command.contains('\n') {
        let first = command.split('\n').next().unwrap_or("").trim();
        if !first.is_empty() {
            return suggestion_for_prefix("Bash", first);
        }
    }
    if let Some(prefix) = get_simple_command_prefix(command) {
        return suggestion_for_prefix("Bash", &prefix);
    }
    suggestion_for_exact_command("Bash", command)
}
/// PowerShellTool/powershellPermissions.ts::suggestionForExactCommand.
/// Multiline and wildcard commands MUST NOT become silently broad rules.
pub fn powershell_exact_suggestions(command: &str) -> Value {
    if command.contains('\n') || command.contains('*') {
        return json!([]);
    }
    suggestion_for_exact_command("PowerShell", command)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn content(v: &Value) -> &str {
        v[0]["rules"][0]["ruleContent"].as_str().unwrap()
    }
    #[test]
    fn original_bash_safe_env_and_two_word_prefix_matrix() {
        assert_eq!(
            get_simple_command_prefix("git commit -m x").as_deref(),
            Some("git commit")
        );
        assert_eq!(
            get_simple_command_prefix("NODE_ENV=prod npm run build").as_deref(),
            Some("npm run")
        );
        assert_eq!(
            get_simple_command_prefix("RUST_BACKTRACE=1 cargo test").as_deref(),
            Some("cargo test")
        );
        assert_eq!(get_simple_command_prefix("PATH=/tmp npm run build"), None);
        assert_eq!(get_simple_command_prefix("MY_VAR=1 npm run build"), None);
        assert_eq!(
            get_simple_command_prefix("DOCKER_HOST=tcp://x docker ps"),
            None
        );
        assert_eq!(get_simple_command_prefix("ls -la"), None);
        assert_eq!(get_simple_command_prefix("cat file.txt"), None);
        assert_eq!(get_simple_command_prefix("chmod 755 file"), None);
        assert_eq!(
            get_simple_command_prefix("git status-porcelain -b").as_deref(),
            Some("git status-porcelain")
        );
        assert_eq!(get_simple_command_prefix(""), None);
    }
    #[test]
    fn original_bash_heredoc_multiline_and_exact_fallback() {
        assert_eq!(
            content(&bash_suggestions("git commit -m test")),
            "git commit:*"
        );
        assert_eq!(
            content(&bash_suggestions("NODE_ENV=prod npm run build")),
            "npm run:*"
        );
        assert_eq!(
            content(&bash_suggestions("MY_VAR=1 npm run build")),
            "MY_VAR=1 npm run build"
        );
        assert_eq!(content(&bash_suggestions("cat <<EOF\nhello\nEOF")), "cat:*");
        assert_eq!(
            content(&bash_suggestions("python3 -c 'print(1)' <<EOF\nhello\nEOF")),
            "python3 -c:*"
        );
        assert_eq!(content(&bash_suggestions("echo hi\nbar")), "echo hi:*");
        assert_eq!(content(&bash_suggestions("ls -la")), "ls -la");
        assert_eq!(extract_prefix_before_heredoc("echo no heredoc"), None);
        assert_eq!(extract_prefix_before_heredoc("<<EOF"), None);
    }
    #[test]
    fn original_ui_only_first_word_filters_wrappers_and_unsafe_env() {
        assert_eq!(get_first_word_prefix("git commit").as_deref(), Some("git"));
        assert_eq!(
            get_first_word_prefix("NODE_ENV=prod npm run build").as_deref(),
            Some("npm")
        );
        for command in [
            "sudo -u root",
            "env bash -c danger",
            "nohup curl url",
            "timeout 10 bash -c x",
            "xargs rm -rf",
            "PATH=/tmp git status",
            "./script.sh",
            "/usr/bin/git status",
            "123 file",
        ] {
            assert_eq!(get_first_word_prefix(command), None, "{command}");
        }
    }
    #[test]
    fn original_powershell_does_not_persist_wildcards_or_multiline() {
        assert!(powershell_exact_suggestions("Remove-Item * -Force")
            .as_array()
            .unwrap()
            .is_empty());
        assert!(powershell_exact_suggestions("Get-ChildItem\nRemove-Item")
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            content(&powershell_exact_suggestions("Get-ChildItem")),
            "Get-ChildItem"
        );
    }
}
