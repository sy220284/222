//! `src/commands/color/color.ts` and the session-scoped color state.
//! The original command validates before writing, persists the literal
//! `default` sentinel on reset, and refuses a teammate's own recoloring.
use crate::session::Session;
use anyhow::Result;
use std::path::Path;

pub const COLORS: [&str; 8] = [
    "red", "blue", "green", "yellow", "purple", "orange", "pink", "cyan",
];
const RESET_ALIASES: [&str; 5] = ["default", "reset", "none", "gray", "grey"];

/// Color changes are persisted before being reported to the caller.
/// A write error leaves the in-memory state unchanged.
pub fn change_color(session: &mut Session, root: &Path, args: &str) -> Result<String> {
    if crate::session_control::is_teammate_session(session) {
        return Ok("Cannot set color: This session is a swarm teammate. Teammate colors are assigned by the team leader.".into());
    }
    let value = args.trim().to_ascii_lowercase();
    let available = || format!("{}, default", COLORS.join(", "));
    if value.is_empty() {
        return Ok(format!(
            "Please provide a color. Available colors: {}",
            available()
        ));
    }
    let reset = RESET_ALIASES.contains(&value.as_str());
    if !reset && !COLORS.contains(&value.as_str()) {
        return Ok(format!(
            "Invalid color \"{value}\". Available colors: {}",
            available()
        ));
    }
    let previous = session
        .agent_color
        .replace(if reset { "default" } else { &value }.to_owned());
    if let Err(error) = session.save(root) {
        session.agent_color = previous;
        return Err(error);
    }
    if reset {
        Ok("Session color reset to default".into())
    } else {
        Ok(format!("Session color set to: {value}"))
    }
}

/// CLI equivalent of the interactive prompt bar color. Do not emit escape
/// sequences when stdout isn't attached to a terminal (SDK and pipes).
pub fn prompt_prefix(color: Option<&str>, terminal: bool) -> String {
    let code = match color {
        Some("red") => "31",
        Some("blue") => "34",
        Some("green") => "32",
        Some("yellow") => "33",
        Some("purple") => "35",
        Some("orange") => "38;5;208",
        Some("pink") => "38;5;213",
        Some("cyan") => "36",
        _ => return "atlas> ".into(),
    };
    if terminal {
        format!("\u{1b}[{code}matlas> \u{1b}[0m")
    } else {
        "atlas> ".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_before_persisting_and_restores_all_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = Session::fresh();
        assert!(change_color(&mut session, dir.path(), " ")
            .unwrap()
            .contains("Available colors"));
        assert!(change_color(&mut session, dir.path(), "unknown")
            .unwrap()
            .contains("Invalid color"));
        assert!(session.agent_color.is_none());
        assert!(change_color(&mut session, dir.path(), "  PuRpLe  ")
            .unwrap()
            .contains("purple"));
        let loaded = Session::load(dir.path(), &session.id.to_string()).unwrap();
        assert_eq!(loaded.agent_color.as_deref(), Some("purple"));
        for alias in RESET_ALIASES {
            assert_eq!(
                change_color(&mut session, dir.path(), alias).unwrap(),
                "Session color reset to default"
            );
            assert_eq!(
                Session::load(dir.path(), &session.id.to_string())
                    .unwrap()
                    .agent_color
                    .as_deref(),
                Some("default")
            );
        }
    }
    #[test]
    fn teammate_and_failed_save_do_not_mutate_color() {
        let dir = tempfile::tempdir().unwrap();
        let mut teammate = Session::fresh();
        teammate.team_name = Some("alpha".into());
        teammate.agent_name = Some("worker".into());
        assert!(change_color(&mut teammate, dir.path(), "cyan")
            .unwrap()
            .contains("team leader"));
        assert!(teammate.agent_color.is_none());
        let missing = dir.path().join("missing");
        assert!(change_color(&mut Session::fresh(), &missing, "cyan").is_err());
        assert_eq!(prompt_prefix(Some("orange"), false), "atlas> ");
        assert!(prompt_prefix(Some("orange"), true).contains("38;5;208"));
    }
}
