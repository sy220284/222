//! Native Rust port derived from original: src/components/HelpV2/Commands.tsx
//!
//! Commands tab decision core: the option builder (label `/name`,
//! description truncated to `columns - 10` with ellipsis), dedupe by
//! name with first-wins, case-insensitive name sort, and the visible
//! count formula `max(1, floor((maxHeight - 10) / 2))`. Select/Tab
//! header focus behavior is component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct HelpCommand {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HelpOption {
    pub label: String,
    pub value: String,
    pub description: String,
}

/// Visible option count formula (verbatim).
pub fn visible_count(max_height: usize) -> usize {
    (max_height.saturating_sub(10) / 2).max(1)
}

/// Max description width (verbatim: columns - 10, floor 1).
pub fn max_description_width(columns: usize) -> usize {
    (columns.saturating_sub(10)).max(1)
}

fn truncate_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Option list: dedupe (first wins), case-insensitive sort, then build.
pub fn build_command_options(commands: &[HelpCommand], max_width: usize) -> Vec<HelpOption> {
    let mut seen: Vec<&str> = Vec::new();
    let mut deduped: Vec<&HelpCommand> = Vec::new();
    for cmd in commands {
        let lower = cmd.name.to_lowercase();
        if !seen.iter().any(|s| s.to_lowercase() == lower) {
            seen.push(&cmd.name);
            deduped.push(cmd);
        }
    }
    deduped.sort_by_key(|c| c.name.to_lowercase());
    deduped
        .into_iter()
        .map(|cmd| HelpOption {
            label: format!("/{}", cmd.name),
            value: cmd.name.clone(),
            description: truncate_ellipsis(&cmd.description, max_width),
        })
        .collect()
}

/// Empty-state decision.
pub fn commands_view_state(
    commands: &[HelpCommand],
    empty_message: Option<&str>,
) -> Result<Vec<HelpOption>, String> {
    if commands.is_empty() {
        if let Some(msg) = empty_message {
            return Err(msg.to_string());
        }
    }
    Ok(build_command_options(commands, 60))
}

/// Registry view shim.
pub fn helpCommandsShim(_args: &[Value]) -> Value {
    json!(null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str, desc: &str) -> HelpCommand {
        HelpCommand {
            name: name.into(),
            description: desc.into(),
        }
    }

    #[test]
    fn formulas_verbatim() {
        assert_eq!(visible_count(10), 1);
        assert_eq!(visible_count(20), 5);
        assert_eq!(max_description_width(60), 50);
        assert_eq!(max_description_width(5), 1);
    }

    #[test]
    fn dedupe_first_wins_and_case_insensitive_sort() {
        let cmds = vec![
            cmd("mcp", "Manage MCP servers"),
            cmd("agents", "Manage agents"),
            cmd("MCP", "Duplicate"),
            cmd("Add", "Add printer"),
        ];
        let options = build_command_options(&cmds, 60);
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].label, "/Add");
        assert_eq!(options[1].label, "/agents");
        assert_eq!(options[2].label, "/mcp");
        assert_eq!(options[2].description, "Manage MCP servers");
    }

    #[test]
    fn truncation_with_ellipsis() {
        let cmds = vec![cmd("x", &"a".repeat(80))];
        let options = build_command_options(&cmds, 50);
        assert_eq!(options[0].description.chars().count(), 50);
        assert!(options[0].description.ends_with('…'));
    }

    #[test]
    fn empty_state_message() {
        let cmds = vec![cmd("help", "Show help")];
        assert!(commands_view_state(&cmds, None).is_ok());
        assert_eq!(
            commands_view_state(&[], Some("No commands available")),
            Err("No commands available".into())
        );
    }
}
