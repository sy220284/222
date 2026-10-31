//! Shared local editor handling from utils/editor.ts (2.1.88).
//! Launch via argv, never interpolate user-controlled filenames into a shell.
use anyhow::{bail, Context, Result};
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorLaunch {
    Detached,
    Exited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiFamily {
    Code,
    Cursor,
    Windsurf,
    Codium,
    Subl,
    Atom,
    Gedit,
    NotepadPlus,
    Notepad,
}

pub fn classify_gui_editor(binary: &str) -> Option<GuiFamily> {
    let name = Path::new(binary)
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    [
        ("code", GuiFamily::Code),
        ("cursor", GuiFamily::Cursor),
        ("windsurf", GuiFamily::Windsurf),
        ("codium", GuiFamily::Codium),
        ("subl", GuiFamily::Subl),
        ("atom", GuiFamily::Atom),
        ("gedit", GuiFamily::Gedit),
        ("notepad++", GuiFamily::NotepadPlus),
        ("notepad", GuiFamily::Notepad),
    ]
    .into_iter()
    .find_map(|(pattern, family)| name.contains(pattern).then_some(family))
}

fn terminal_goto_line(binary: &str) -> bool {
    let filename = Path::new(binary)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    filename
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|part| matches!(part, "vi" | "vim" | "nvim" | "nano" | "emacs" | "pico" | "micro" | "helix" | "hx"))
}

pub fn parse_editor_command(raw: &str) -> Result<Vec<String>> {
    let mut output = Vec::new();
    let mut token = String::new();
    let mut chars = raw.chars().peekable();
    let mut quote = None;
    let mut active = false;
    while let Some(ch) = chars.next() {
        if ch == '\\' && quote != Some('\'') {
            match chars.peek().copied() {
                Some(next) if next.is_whitespace() || next == '\\' || next == '"' || next == '\'' => {
                    token.push(chars.next().expect("next was checked"));
                    active = true;
                }
                _ => {
                    // Paths like C:\Tools\editor.exe keep literal backslashes.
                    token.push(ch);
                    active = true;
                }
            }
        } else if let Some(open) = quote {
            if ch == open {
                quote = None;
            } else {
                token.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
            active = true;
        } else if ch.is_whitespace() {
            if active {
                output.push(std::mem::take(&mut token));
                active = false;
            }
        } else {
            token.push(ch);
            active = true;
        }
    }
    if quote.is_some() {
        bail!("Editor command contains an unclosed quote");
    }
    if active {
        output.push(token);
    }
    if output.is_empty() || output[0].is_empty() {
        bail!("Editor command is empty");
    }
    Ok(output)
}

fn in_path(binary: &str) -> bool {
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(binary))
            .any(|path| path.is_file())
    })
}

fn find_editor() -> Result<(Vec<String>, &'static str)> {
    for (key, source) in [("VISUAL", "$VISUAL"), ("EDITOR", "$EDITOR")] {
        if let Some(raw) = env::var_os(key) {
            let raw = raw.to_string_lossy();
            if !raw.trim().is_empty() {
                return Ok((parse_editor_command(raw.trim())?, source));
            }
        }
    }
    #[cfg(windows)]
    {
        Ok((vec!["notepad.exe".to_owned()], "default"))
    }
    #[cfg(not(windows))]
    {
        for binary in ["code", "vi", "nano"] {
            if in_path(binary) {
                return Ok((vec![binary.to_owned()], "default"));
            }
        }
        bail!("No external editor found; set VISUAL or EDITOR")
    }
}

pub fn goto_arguments(binary: &str, path: &Path, line: Option<u32>) -> Vec<String> {
    let filename = path.to_string_lossy().into_owned();
    let line = line.filter(|line| *line != 0);
    if let Some(gui) = classify_gui_editor(binary) {
        return match (gui, line) {
            (GuiFamily::Code | GuiFamily::Cursor | GuiFamily::Windsurf | GuiFamily::Codium, Some(line)) => {
                vec!["-g".to_owned(), format!("{filename}:{line}")]
            }
            (GuiFamily::Subl, Some(line)) => vec![format!("{filename}:{line}")],
            _ => vec![filename],
        };
    }
    match line {
        Some(line) if terminal_goto_line(binary) => vec![format!("+{line}"), filename],
        _ => vec![filename],
    }
}

pub fn open_file(path: &Path, line: Option<u32>) -> Result<(&'static str, EditorLaunch)> {
    let (argv, source) = find_editor()?;
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    command.args(goto_arguments(&argv[0], path, line));
    if classify_gui_editor(&argv[0]).is_some() {
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        // Source launches a GUI editor without blocking the active REPL.
        command
            .spawn()
            .with_context(|| format!("Launch GUI editor {}", argv[0]))?;
        return Ok((source, EditorLaunch::Detached));
    }
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Launch terminal editor {}", argv[0]))?;
    if !status.success() {
        bail!("Editor exited with status {status}");
    }
    Ok((source, EditorLaunch::Exited))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_classification_uses_binary_basename_not_parent_directory() {
        assert_eq!(classify_gui_editor("/some/code/bin/nvim"), None);
        assert_eq!(classify_gui_editor("/usr/bin/code-insiders"), Some(GuiFamily::Code));
        assert_eq!(classify_gui_editor("cursor"), Some(GuiFamily::Cursor));
        assert_eq!(classify_gui_editor("notepad.exe"), Some(GuiFamily::Notepad));
    }

    #[test]
    fn editor_line_arguments_match_gui_and_terminal_families() {
        let path = Path::new("/tmp/a b;$(unsafe).rs");
        assert_eq!(
            goto_arguments("code-insiders", path, Some(42)),
            vec!["-g", "/tmp/a b;$(unsafe).rs:42"]
        );
        assert_eq!(goto_arguments("subl", path, Some(3)), vec!["/tmp/a b;$(unsafe).rs:3"]);
        assert_eq!(
            goto_arguments("nvim", path, Some(3)),
            vec!["+3", "/tmp/a b;$(unsafe).rs"]
        );
        assert_eq!(goto_arguments("notepad", path, Some(3)), vec!["/tmp/a b;$(unsafe).rs"]);
        assert_eq!(goto_arguments("vim", path, Some(0)), vec!["/tmp/a b;$(unsafe).rs"]);
    }

    #[test]
    fn editor_argv_preserves_shell_metacharacters_as_literal_arguments() {
        assert_eq!(
            parse_editor_command(r#""/some path/code" --wait"#).unwrap(),
            vec!["/some path/code", "--wait"]
        );
        assert_eq!(
            parse_editor_command(r#"vi -u 'custom rc'"#).unwrap(),
            vec!["vi", "-u", "custom rc"]
        );
        assert_eq!(
            parse_editor_command(r#""C:\Program Files\Editor\editor.exe" --wait"#).unwrap(),
            vec![r"C:\Program Files\Editor\editor.exe", "--wait"]
        );
        assert_eq!(
            parse_editor_command("code --wait '; touch /tmp/x'").unwrap(),
            vec!["code", "--wait", "; touch /tmp/x"]
        );
        assert!(parse_editor_command("'unterminated").is_err());
        assert!(parse_editor_command("").is_err());
    }
}
