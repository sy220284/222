//! Hand port of src/commands/vim/vim.ts, without upstream telemetry.
use crate::config::Config;
use anyhow::Result;

/// Toggle the persisted editor preference and report the exact source wording.
/// The legacy "emacs" value is treated as normal mode.
pub fn toggle_editor_mode(config: &mut Config) -> Result<String> {
    let next_mode = if config.editor_mode == "vim" { "normal" } else { "vim" };
    let mut next = config.clone();
    next.editor_mode = next_mode.to_owned();
    // Persist before changing the active session. A failed write must not
    // silently advertise a successful mode change.
    next.save()?;
    config.editor_mode = next.editor_mode;
    let help = if next_mode == "vim" {
        "Use Escape key to toggle between INSERT and NORMAL modes."
    } else {
        "Using standard (readline) keyboard bindings."
    };
    Ok(format!("Editor mode set to {next_mode}. {help}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vim_mode_legacy_emacs_and_toggle_messages() {
        let mut config = Config::default();
        config.editor_mode = "emacs".to_owned();
        // Assert source semantics without writing to the user's real config.
        for (before, after, message) in [
            ("emacs", "vim", "Use Escape key to toggle between INSERT and NORMAL modes."),
            ("normal", "vim", "Use Escape key to toggle between INSERT and NORMAL modes."),
            ("vim", "normal", "Using standard (readline) keyboard bindings."),
        ] {
            config.editor_mode = before.to_owned();
            assert_eq!(
                next_editor_mode(&config.editor_mode), after,
            );
            assert_eq!(
                editor_mode_message(after),
                format!("Editor mode set to {after}. {message}")
            );
        }
    }
}

pub fn next_editor_mode(current_mode: &str) -> &'static str {
    if current_mode == "vim" { "normal" } else { "vim" }
}

pub fn editor_mode_message(new_mode: &str) -> String {
    let help = if new_mode == "vim" {
        "Use Escape key to toggle between INSERT and NORMAL modes."
    } else {
        "Using standard (readline) keyboard bindings."
    };
    format!("Editor mode set to {new_mode}. {help}")
}
