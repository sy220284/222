//! Native Rust port derived from original: src/components/messages/teamMemCollapsed.tsx
//!
//! Team-memory collapsed count parts. checkHasTeamMemOps is verbatim;
//! the verb decision (capitalized for the first part of the sentence or
//! active groups: "Recalling"/"recalling", "Recalled"/"recalled") and
//! the three-part text assembly with singular/plural "memory/memories"
//! are extracted as pure functions — the React memoization scaffolding
//! is component-domain and intentionally dropped.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, Copy, Default)]
pub struct TeamMemCounts {
    pub read: i64,
    pub search: i64,
    pub write: i64,
}

impl TeamMemCounts {
    pub fn from_message(message: &Value) -> Self {
        let g = |k: &str| message.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
        Self {
            read: g("teamMemoryReadCount"),
            search: g("teamMemorySearchCount"),
            write: g("teamMemoryWriteCount"),
        }
    }

    pub fn any(&self) -> bool {
        self.read > 0 || self.search > 0 || self.write > 0
    }
}

/// `checkHasTeamMemOps` verbatim.
pub fn check_has_team_mem_ops(message: &Value) -> bool {
    TeamMemCounts::from_message(message).any()
}

/// Verb decision: active groups use present tense, completed use past;
/// the first part of the sentence is capitalized.
pub fn verb_for(kind: &str, is_active_group: bool, first_part: bool) -> &'static str {
    match (kind, is_active_group, first_part) {
        ("read", true, true) => "Recalling",
        ("read", true, false) => "recalling",
        ("read", false, true) => "Recalled",
        ("read", false, false) => "recalled",
        ("search", true, true) => "Searching",
        ("search", true, false) => "searching",
        ("search", false, true) => "Searched",
        ("search", false, false) => "searched",
        ("write", true, true) => "Writing",
        ("write", true, false) => "writing",
        ("write", false, true) => "Wrote",
        ("write", false, false) => "wrote",
        _ => "",
    }
}

/// Part text for reads: "{verb} {n} team {memory|memories}".
pub fn read_part_text(verb: &str, count: i64) -> String {
    let noun = if count == 1 { "memory" } else { "memories" };
    format!("{verb} {count} team {noun}")
}

/// Part text for searches: "{verb} team memories".
pub fn search_part_text(verb: &str) -> String {
    format!("{verb} team memories")
}

/// Part text for writes: "{verb} {n} memories".
pub fn write_part_text(verb: &str, count: i64) -> String {
    let noun = if count == 1 { "memory" } else { "memories" };
    format!("{verb} {count} {noun}")
}

/// Full collapsed suffix for a message (comma-joined parts), or None
/// when no team-memory ops are present (view returns null upstream).
pub fn collapsed_suffix(
    message: &Value,
    is_active_group: bool,
    has_preceding_parts: bool,
) -> Option<String> {
    let counts = TeamMemCounts::from_message(message);
    if !counts.any() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    let mut count = if has_preceding_parts { 1 } else { 0 };
    if counts.read > 0 {
        let verb = verb_for("read", is_active_group, count == 0);
        parts.push(read_part_text(verb, counts.read));
        count += 1;
    }
    if counts.search > 0 {
        let verb = verb_for("search", is_active_group, count == 0);
        parts.push(search_part_text(verb));
        count += 1;
    }
    if counts.write > 0 {
        let verb = verb_for("write", is_active_group, count == 0);
        parts.push(write_part_text(verb, counts.write));
    }
    Some(parts.join(", "))
}

/// Registry view shim.
pub fn checkHasTeamMemOps(args: &[Value]) -> Value {
    match args.first() {
        Some(m) => json!(check_has_team_mem_ops(m)),
        None => json!(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn has_ops_checks_three_counts() {
        assert!(!check_has_team_mem_ops(&json!({})));
        assert!(check_has_team_mem_ops(&json!({"teamMemoryReadCount": 1})));
        assert!(check_has_team_mem_ops(&json!({"teamMemoryWriteCount": 2})));
    }

    #[test]
    fn verb_capitalization_rules() {
        assert_eq!(verb_for("read", true, true), "Recalling");
        assert_eq!(verb_for("read", true, false), "recalling");
        assert_eq!(verb_for("read", false, true), "Recalled");
        assert_eq!(verb_for("search", false, false), "searched");
        assert_eq!(verb_for("write", false, true), "Wrote");
    }

    #[test]
    fn part_text_singular_plural() {
        assert_eq!(read_part_text("Recalled", 1), "Recalled 1 team memory");
        assert_eq!(read_part_text("recalled", 3), "recalled 3 team memories");
        assert_eq!(search_part_text("Searched"), "Searched team memories");
        assert_eq!(write_part_text("Wrote", 2), "Wrote 2 memories");
    }

    #[test]
    fn suffix_joins_with_commas_and_capitalizes_first() {
        let msg = json!({"teamMemoryReadCount": 2, "teamMemorySearchCount": 1});
        let suffix = collapsed_suffix(&msg, false, false).unwrap();
        assert_eq!(suffix, "Recalled 2 team memories, searched team memories");
        // Preceding parts shift capitalization off the first segment.
        let suffix2 = collapsed_suffix(&msg, false, true).unwrap();
        assert!(suffix2.starts_with("recalled"));
    }

    #[test]
    fn no_ops_returns_none() {
        assert!(collapsed_suffix(&json!({}), false, false).is_none());
    }
}
