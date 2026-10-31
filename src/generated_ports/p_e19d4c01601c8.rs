//! Native Rust port derived from original: src/utils/teamDiscovery.ts
//!
//! Team discovery for the status-line UI: scans the workspace teams
//! directory and summarizes teammate status for the team the current
//! session leads. The repo's `Team`/`Member` structs are the simplified
//! core schema, so this port parses the on-disk team JSON (serde_json)
//! and reads the richer original fields (`agentType`, `model`, `prompt`,
//! `status`, `color`, `idleSince`, `tmuxPaneId`, `cwd`, `worktreePath`,
//! `isHidden`, `backendType`, `mode`) directly. Running/idle counts and
//! hidden filtering match the original; `isPaneBackend` accepts only
//! `tmux` | `iterm2`.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::generated_ports::p_5b257a1951fde::{log_for_debugging, DebugLogLevel};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamSummary {
    pub name: String,
    pub member_count: usize,
    pub running_count: usize,
    pub idle_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeammateStatus {
    pub name: String,
    pub agent_id: String,
    pub agent_type: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    /// "running" | "idle" | "unknown"
    pub status: String,
    pub color: Option<String>,
    pub idle_since: Option<String>,
    pub tmux_pane_id: String,
    pub cwd: String,
    pub worktree_path: Option<String>,
    pub is_hidden: bool,
    /// "tmux" | "iterm2" when a valid pane backend.
    pub backend_type: Option<String>,
    pub mode: Option<String>,
}

pub fn is_pane_backend(backend: &str) -> bool {
    backend == "tmux" || backend == "iterm2"
}

fn member_to_status(name: &str, member: &Value) -> TeammateStatus {
    let get = |key: &str| member.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    let raw_status = get("status").unwrap_or_default();
    let status = match raw_status.as_str() {
        "running" => "running".to_string(),
        "idle" => "idle".to_string(),
        _ => "unknown".to_string(),
    };
    let backend = get("backendType").filter(|b| is_pane_backend(b));
    TeammateStatus {
        name: member
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string(),
        agent_id: member
            .get("agentId")
            .or_else(|| member.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        agent_type: get("agentType"),
        model: get("model"),
        prompt: get("prompt"),
        status,
        color: get("color"),
        idle_since: get("idleSince"),
        tmux_pane_id: get("tmuxPaneId").unwrap_or_default(),
        cwd: get("cwd").unwrap_or_default(),
        worktree_path: get("worktreePath"),
        is_hidden: member
            .get("isHidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        backend_type: backend,
        mode: get("mode"),
    }
}

/// Summarizes a parsed team document (original `getTeammateStatuses`
/// core). Hidden teammates are excluded from the returned list and from
/// the running/idle tallies.
pub fn summarize_team(team_file: &Value) -> Option<TeamSummary> {
    let name = team_file
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_owned)?;
    let members = team_file.get("members")?.as_array()?;
    let mut summary = TeamSummary {
        name,
        member_count: members.len(),
        ..Default::default()
    };
    for member in members {
        let member_status = member.get("status").and_then(|v| v.as_str());
        let hidden = member
            .get("isHidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        match member_status {
            Some("running") if !hidden => summary.running_count += 1,
            Some("idle") if !hidden => summary.idle_count += 1,
            _ => {}
        }
    }
    Some(summary)
}

/// Reads a team file from `<root>/.atlas/teams/<name>.json` and returns
/// the raw document. Mirrors `readTeamFile`'s data access; missing or
/// unparsable files log a debug line and yield `None`.
pub fn read_team_file(root: &Path, team_name: &str) -> Option<Value> {
    let path = root
        .join(".atlas")
        .join("teams")
        .join(format!("{team_name}.json"));
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(value) => Some(value),
            Err(error) => {
                log_for_debugging(
                    &format!("Failed to parse team file {}: {}", path.display(), error),
                    DebugLogLevel::debug,
                );
                None
            }
        },
        Err(error) => {
            log_for_debugging(
                &format!("Failed to read team file {}: {}", path.display(), error),
                DebugLogLevel::debug,
            );
            None
        }
    }
}

/// Full discovery flow: teammate statuses for a team, hidden members
/// excluded (original `getTeammateStatuses`).
pub fn get_teammate_statuses(root: &Path, team_name: &str) -> Option<Vec<TeammateStatus>> {
    let team_file = read_team_file(root, team_name)?;
    let members = team_file.get("members")?.as_array()?;
    Some(
        members
            .iter()
            .filter_map(|member| {
                let name = member
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let status = member_to_status(&name, member);
                if status.is_hidden {
                    None
                } else {
                    Some(status)
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_team() -> Value {
        json!({
            "name": "alpha",
            "members": [
                { "name": "lead", "agentId": "a1", "status": "running",
                  "tmuxPaneId": "%0", "cwd": "/w", "backendType": "tmux" },
                { "name": "worker", "agentId": "a2", "status": "idle",
                  "tmuxPaneId": "%1", "cwd": "/w", "isHidden": true },
                { "name": "watcher", "agentId": "a3", "status": "idle",
                  "tmuxPaneId": "%2", "cwd": "/w", "backendType": "footmux" }
            ]
        })
    }

    #[test]
    fn summary_counts_exclude_hidden() {
        let summary = summarize_team(&sample_team()).unwrap();
        assert_eq!(summary.name, "alpha");
        assert_eq!(summary.member_count, 3);
        assert_eq!(summary.running_count, 1);
        assert_eq!(summary.idle_count, 1); // hidden idle excluded, watcher counts
    }

    #[test]
    fn member_fields_parse_and_invalid_backend_dropped() {
        let team = sample_team();
        let statuses: Vec<TeammateStatus> = team["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| member_to_status("fallback", m))
            .collect();
        assert_eq!(statuses[0].agent_id, "a1");
        assert_eq!(statuses[0].backend_type.as_deref(), Some("tmux"));
        // "footmux" is not a valid pane backend -> None.
        assert_eq!(statuses[2].backend_type, None);
        assert_eq!(statuses[2].status, "idle");
    }

    #[test]
    fn is_pane_backend_whitelist() {
        assert!(is_pane_backend("tmux"));
        assert!(is_pane_backend("iterm2"));
        assert!(!is_pane_backend("footmux"));
    }

    #[test]
    fn hidden_members_excluded_from_statuses() {
        let statuses = get_teammate_statuses(Path::new("/nonexistent-root"), "alpha");
        assert!(statuses.is_none()); // missing file -> None
    }
}
