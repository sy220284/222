//! Native Rust port derived from original: src/types/ids.ts
//!
//! Branded identifiers for session and agent IDs. TypeScript's compile-time
//! brands become Rust newtype wrappers (`SessionId`/`AgentId`), giving the
//! same mix-up prevention at the type level. `toAgentId` validates the
//! `a` + optional `<label>-` + 16-hex format produced by `createAgentId`.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl SessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AgentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Cast a raw string to SessionId (original `asSessionId`).
pub fn as_session_id(id: String) -> SessionId {
    SessionId(id)
}

/// Cast a raw string to AgentId (original `asAgentId`).
pub fn as_agent_id(id: String) -> AgentId {
    AgentId(id)
}

/// Generate a fresh AgentId: `a` + optional `<label>-` + 16 hex chars.
pub fn create_agent_id(label: Option<&str>) -> String {
    let hex: String = std::iter::repeat_with(|| {
        let b: u8 = rand_byte();
        format!("{b:02x}").chars().next().unwrap()
    })
    .take(16)
    .collect();
    match label {
        Some(l) if !l.is_empty() => format!("a{l}-{hex}"),
        _ => format!("a{hex}"),
    }
}

fn rand_byte() -> u8 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    (n ^ std::process::id() as u64) as u8
}

const AGENT_ID_PATTERN_Prefix: &str = "a";
/// Validate and brand a string as AgentId: `a` + optional `<label>-` + 16 hex.
pub fn to_agent_id(s: &str) -> Option<AgentId> {
    let rest = s.strip_prefix('a')?;
    let hex_part = match rest.split_once('-') {
        Some((_label, hex)) => hex,
        None => rest,
    };
    if hex_part.len() == 16 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(AgentId(s.to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_pattern_matches_generated_shape() {
        let id = create_agent_id(None);
        assert!(to_agent_id(id.as_str()).is_some(), "bare hex form accepted");
        let labeled = create_agent_id(Some("worker"));
        assert!(
            to_agent_id(labeled.as_str()).is_some(),
            "labeled form accepted"
        );
        assert!(labeled.as_str().starts_with("aworker-"));
    }

    #[test]
    fn to_agent_id_rejects_non_matching() {
        assert!(to_agent_id("team-boss").is_none());
        assert!(to_agent_id("a12").is_none());
        assert!(to_agent_id("aZZZZZZZZZZZZZZZZ").is_none());
        assert!(to_agent_id("b1234567890abcdef").is_none());
    }

    #[test]
    fn branding_types_hold_values() {
        let s = as_session_id("sess-1".into());
        assert_eq!(s.as_str(), "sess-1");
        let a = as_agent_id("a1234567890abcdef".into());
        assert_eq!(a.as_str(), "a1234567890abcdef");
    }
}

/// Registry-view wrappers preserving the original export names.
pub fn asSessionId(args: &[Value]) -> Value {
    let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
    json!(as_session_id(id.to_string()))
}

pub fn asAgentId(args: &[Value]) -> Value {
    let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
    json!(as_agent_id(id.to_string()))
}

pub fn toAgentId(args: &[Value]) -> Value {
    let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
    match to_agent_id(id) {
        Some(a) => json!(a),
        None => Value::Null,
    }
}
