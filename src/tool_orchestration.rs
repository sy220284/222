//! Rust port of `src/services/tools/toolOrchestration.ts` (2.1.88).
//!
//! Tool calls from one assistant turn are partitioned into batches: each
//! batch is either a single non-read-only tool or a run of consecutive
//! read-only tools. Read-only batches run concurrently up to
//! `ATLAS_MAX_TOOL_USE_CONCURRENCY` (default 10, mirroring
//! `the original_CODE_MAX_TOOL_USE_CONCURRENCY`), while unsafe tools run serially
//! so their context effects are observed before the next call starts.
//!
//! The `isConcurrencySafe` table mirrors the original per-tool definitions
//! with a fail-closed default (unknown/unported tools are never concurrent):
//! the original marks Read/Glob/Grep/WebFetch/WebSearch/ToolSearch and a set
//! of task/plan helpers as safe; tools whose Rust execution still requires
//! exclusive agent access (subagents, task graph, MCP) stay conservative
//! until their executor paths are made shared.

use crate::protocol::ToolCall;
use std::collections::HashSet;
use std::sync::OnceLock;

/// `getMaxToolUseConcurrency` — `parseInt(env) || 10`. Empty strings, non
/// numbers and zero all fall back to 10 exactly like the JavaScript parse.
pub fn max_tool_use_concurrency() -> usize {
    std::env::var("ATLAS_MAX_TOOL_USE_CONCURRENCY")
        .ok()
        .map(|raw| parse_js_concurrency(&raw))
        .unwrap_or(10)
}

fn parse_js_concurrency(raw: &str) -> usize {
    // JS parseInt("3threads", 10) returns 3. An empty/non-numeric/zero
    // value uses the original fallback. Negative/overflow worker counts are
    // invalid on this Rust executor and retain the safe default.
    let value = raw.trim_start();
    let value = value.strip_prefix('+').unwrap_or(value);
    if value.starts_with('-') {
        return 10;
    }
    let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .unwrap_or(10)
}

fn concurrency_safe_tools() -> &'static HashSet<&'static str> {
    static SAFE: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SAFE.get_or_init(|| {
        HashSet::from([
            "read_file",
            "glob_files",
            "grep_files",
            "web_fetch",
            "web_search",
            "tool_search",
        ])
    })
}

/// `tool.isConcurrencySafe(input)` — the Rust tool surface has no per-tool
/// safe predicates yet, so the table holds the tools whose original
/// definitions are unconditionally safe. Schema validation is unnecessary
/// here: the main loop already rejects calls whose arguments do not parse
/// before dispatch, and the original treats a failed safe-parse as unsafe.
pub fn is_concurrency_safe(tool_name: &str) -> bool {
    concurrency_safe_tools().contains(crate::tool_names::canonical_name(tool_name))
}

/// The source RemoteTrigger marks tool requests as concurrent-safe. Local
/// mutations use a single durable file and therefore stay serialized; pure
/// list/get calls can join other read-only work without waiting behind them.
pub fn is_concurrency_safe_call(call: &ToolCall) -> bool {
    if crate::tool_names::canonical_name(&call.function.name) == "LocalTrigger" {
        let Ok(input) = serde_json::from_str::<serde_json::Value>(&call.function.arguments) else {
            return false;
        };
        if crate::local_triggers::validate_model_input(&input).is_err() {
            return false;
        }
        return matches!(
            input.get("action").and_then(serde_json::Value::as_str),
            Some("list" | "get")
        );
    }
    is_concurrency_safe(&call.function.name)
}

/// `Batch` — a run of consecutive safe tools, or a single unsafe tool.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolBatch {
    /// Multiple consecutive read-only tools; may run concurrently.
    Safe(Vec<ToolCall>),
    /// One non-read-only tool; must run serially.
    Unsafe(ToolCall),
}

/// `partitionToolCalls` — reduce the turn's tool calls into batches.
/// Safe tools merge into the previous batch when that batch is also safe;
/// anything else (unsafe, unknown, schema failure in the original) opens a
/// new singleton batch, so unsafe tools always execute alone in order.
pub fn partition_tool_calls(calls: Vec<ToolCall>) -> Vec<ToolBatch> {
    let mut batches: Vec<ToolBatch> = Vec::new();
    for call in calls {
        let safe = is_concurrency_safe_call(&call);
        if safe {
            if let Some(ToolBatch::Safe(blocks)) = batches.last_mut() {
                blocks.push(call);
                continue;
            }
        }
        if safe {
            batches.push(ToolBatch::Safe(vec![call]));
        } else {
            batches.push(ToolBatch::Unsafe(call));
        }
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FunctionCall, ToolCall};

    fn call(name: &str, id: &str) -> ToolCall {
        ToolCall {
            id: id.to_owned(),
            kind: "function".to_owned(),
            function: FunctionCall {
                name: name.to_owned(),
                arguments: "{}".to_owned(),
            },
        }
    }

    #[test]
    fn javascript_parse_int_accepts_numeric_prefix() {
        assert_eq!(parse_js_concurrency("3threads"), 3);
        assert_eq!(parse_js_concurrency(" +004 trailing"), 4);
        assert_eq!(parse_js_concurrency("0x10"), 10);
        assert_eq!(parse_js_concurrency("-5"), 10);
        assert_eq!(parse_js_concurrency("garbage"), 10);
    }

    #[test]
    fn local_trigger_reads_join_safe_batches_and_mutations_stay_serial() {
        let call = |action: &str, id: &str| ToolCall {
            id: id.to_owned(),
            kind: "function".to_owned(),
            function: FunctionCall {
                name: "LocalTrigger".into(),
                arguments: serde_json::json!({
                    "action":action,
                    "trigger_id":(action != "list").then_some("manual"),
                    "body":(action == "create" || action == "update")
                        .then_some(serde_json::json!({"prompt":"run locally"})),
                })
                .to_string(),
            },
        };
        // Remove absent optional fields: the tool's strict schema rejects
        // null optionals, as does the original Zod strictObject.
        let normalized = |action: &str, id: &str| {
            let mut call = call(action, id);
            let mut input: serde_json::Value =
                serde_json::from_str(&call.function.arguments).unwrap();
            let fields = input.as_object_mut().unwrap();
            fields.retain(|_, value| !value.is_null());
            if action == "create" {
                fields.remove("trigger_id");
            }
            call.function.arguments = input.to_string();
            call
        };
        let list = normalized("list", "l");
        let get = normalized("get", "g");
        let create = normalized("create", "c");
        assert!(is_concurrency_safe_call(&list));
        assert!(is_concurrency_safe_call(&get));
        assert!(!is_concurrency_safe_call(&create));
        let batches = partition_tool_calls(vec![list, get, create]);
        assert!(matches!(&batches[0], ToolBatch::Safe(v) if v.len() == 2));
        assert!(matches!(&batches[1], ToolBatch::Unsafe(_)));
    }

    #[test]
    fn read_only_table_matches_original_definitions() {
        for name in [
            "read_file",
            "glob_files",
            "grep_files",
            "web_fetch",
            "web_search",
            "tool_search",
        ] {
            assert!(is_concurrency_safe(name), "{name} should be safe");
        }
        // Original marks Edit/Write/Bash-family/task/subagent tools unsafe;
        // the Rust executor still requires exclusive access, fail closed.
        for name in [
            "edit_file",
            "write_file",
            "notebook_edit",
            "run_command",
            "PowerShell",
            "spawn_agent",
            "spawn_team",
            "task_create",
            "task_update",
            "TodoWrite",
            "Skill",
            "unknown_tool",
        ] {
            assert!(!is_concurrency_safe(name), "{name} must stay serial");
        }
    }

    #[test]
    fn consecutive_safe_tools_merge_into_one_batch() {
        let batches = partition_tool_calls(vec![
            call("Read", "a"),
            call("Grep", "b"),
            call("Glob", "c"),
        ]);
        assert_eq!(batches.len(), 1);
        match &batches[0] {
            ToolBatch::Safe(blocks) => {
                assert_eq!(blocks.len(), 3);
                assert_eq!(blocks[0].id, "a");
                assert_eq!(blocks[2].id, "c");
            }
            other => panic!("expected safe batch, got {other:?}"),
        }
    }

    #[test]
    fn unsafe_tool_splits_batches_on_both_sides() {
        let batches = partition_tool_calls(vec![
            call("Read", "a"),
            call("Edit", "b"),
            call("Grep", "c"),
        ]);
        assert_eq!(batches.len(), 3);
        assert!(matches!(&batches[0], ToolBatch::Safe(v) if v.len() == 1));
        assert!(matches!(&batches[1], ToolBatch::Unsafe(c) if c.id == "b"));
        assert!(matches!(&batches[2], ToolBatch::Safe(v) if v.len() == 1));
    }

    #[test]
    fn trailing_safe_batch_is_split_from_leading_safe_batch() {
        let batches = partition_tool_calls(vec![
            call("Glob", "a"),
            call("Glob", "b"),
            call("run_command", "c"),
            call("Glob", "d"),
        ]);
        assert_eq!(batches.len(), 3);
        match &batches[0] {
            ToolBatch::Safe(blocks) => assert_eq!(blocks.len(), 2),
            other => panic!("expected leading safe batch, got {other:?}"),
        }
        assert!(matches!(&batches[1], ToolBatch::Unsafe(_)));
        match &batches[2] {
            ToolBatch::Safe(blocks) => assert_eq!(blocks.len(), 1),
            other => panic!("expected trailing safe batch, got {other:?}"),
        }
    }

    #[test]
    fn empty_input_yields_no_batches() {
        assert!(partition_tool_calls(Vec::new()).is_empty());
    }

    #[test]
    fn concurrency_limit_follows_js_parse_or_ten() {
        // SAFETY: tests run single-threaded per env var; each case resets.
        for (raw, expected) in [
            (Some("3"), 3usize),
            (Some(" 4 "), 4),
            (Some("0"), 10),
            (Some("abc"), 10),
            (Some(""), 10),
            (Some("-5"), 10),
            (None, 10),
        ] {
            match raw {
                Some(v) => std::env::set_var("ATLAS_MAX_TOOL_USE_CONCURRENCY", v),
                None => std::env::remove_var("ATLAS_MAX_TOOL_USE_CONCURRENCY"),
            }
            assert_eq!(max_tool_use_concurrency(), expected, "raw={raw:?}");
        }
        std::env::remove_var("ATLAS_MAX_TOOL_USE_CONCURRENCY");
    }
}
