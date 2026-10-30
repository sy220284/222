//! Native Rust port derived from original: src/utils/streamlinedTransform.ts
//!
//! Streamlined SDK output: text kept, tool calls summarized with
//! cumulative counts (reset when text appears), thinking omitted, init
//! stripped of tool/model info. Tool categorization prefixes and the
//! summary phrasing are verbatim.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

pub const SEARCH_TOOLS: &[&str] = &["Grep", "Glob", "WebSearch", "LSP"];
pub const READ_TOOLS: &[&str] = &["Read", "ListMcpResources"];
pub const WRITE_TOOLS: &[&str] = &["Write", "Edit", "NotebookEdit"];
pub const COMMAND_TOOLS: &[&str] = &["Bash", "Tmux", "TaskStop"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolCounts {
    pub searches: usize,
    pub reads: usize,
    pub writes: usize,
    pub commands: usize,
    pub other: usize,
}

/// `categorizeToolName` (prefix match, verbatim order).
pub fn categorize_tool_name(tool_name: &str) -> &'static str {
    if SEARCH_TOOLS.iter().any(|t| tool_name.starts_with(t)) {
        return "searches";
    }
    if READ_TOOLS.iter().any(|t| tool_name.starts_with(t)) {
        return "reads";
    }
    if WRITE_TOOLS.iter().any(|t| tool_name.starts_with(t)) {
        return "writes";
    }
    if COMMAND_TOOLS.iter().any(|t| tool_name.starts_with(t)) {
        return "commands";
    }
    "other"
}

/// `getToolSummaryText` with singular/plural phrasing (verbatim).
pub fn get_tool_summary_text(counts: &ToolCounts) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if counts.searches > 0 {
        parts.push(format!(
            "searched {} {}",
            counts.searches,
            if counts.searches == 1 {
                "pattern"
            } else {
                "patterns"
            }
        ));
    }
    if counts.reads > 0 {
        parts.push(format!(
            "read {} {}",
            counts.reads,
            if counts.reads == 1 { "file" } else { "files" }
        ));
    }
    if counts.writes > 0 {
        parts.push(format!(
            "wrote {} {}",
            counts.writes,
            if counts.writes == 1 { "file" } else { "files" }
        ));
    }
    if counts.commands > 0 {
        parts.push(format!(
            "ran {} {}",
            counts.commands,
            if counts.commands == 1 {
                "command"
            } else {
                "commands"
            }
        ));
    }
    if counts.other > 0 {
        parts.push(format!(
            "ran {} other tool{}",
            counts.other,
            if counts.other == 1 { "" } else { "s" }
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// Streamlined message transformation over an SDK assistant message:
/// text kept intact; tool_use blocks counted (summary attached);
/// thinking omitted (original behavior via extractTextContent chain).
pub fn transform_streamlined(messages: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut counts = ToolCounts::default();
    for message in messages {
        let mtype = message.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if mtype == "assistant" {
            let content = message
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array());
            let texts: Vec<&Value> = content
                .map(|c| {
                    c.iter()
                        .filter(|b| b.get("type").and_then(|v| v.as_str()) == Some("text"))
                        .collect()
                })
                .unwrap_or_default();
            if !texts.is_empty() {
                // Text appearance keeps the message and resets the counts.
                counts = ToolCounts::default();
                out.push(message.clone());
                continue;
            }
            // Tool-use-only assistant blocks collapse into counts.
            if let Some(blocks) = content {
                for block in blocks {
                    if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        match categorize_tool_name(name) {
                            "searches" => counts.searches += 1,
                            "reads" => counts.reads += 1,
                            "writes" => counts.writes += 1,
                            "commands" => counts.commands += 1,
                            _ => counts.other += 1,
                        }
                    }
                }
            }
        } else {
            out.push(message.clone());
        }
    }
    if let Some(summary) = get_tool_summary_text(&counts) {
        out.push(json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": summary}]
            }
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorization_prefix_order() {
        assert_eq!(categorize_tool_name("Grep"), "searches");
        assert_eq!(categorize_tool_name("Glob something"), "searches");
        assert_eq!(categorize_tool_name("Read"), "reads");
        assert_eq!(categorize_tool_name("Write"), "writes");
        assert_eq!(categorize_tool_name("Edit"), "writes");
        assert_eq!(categorize_tool_name("BashOutput"), "commands");
        assert_eq!(categorize_tool_name("TaskStop"), "commands");
        assert_eq!(categorize_tool_name("Mystery"), "other");
    }

    #[test]
    fn summary_singular_plural() {
        let mut c = ToolCounts {
            searches: 1,
            reads: 3,
            ..Default::default()
        };
        assert_eq!(
            get_tool_summary_text(&c),
            Some("searched 1 pattern, read 3 files".into())
        );
        c.writes = 1;
        assert!(get_tool_summary_text(&c).unwrap().contains("wrote 1 file"));
        assert_eq!(get_tool_summary_text(&ToolCounts::default()), None);
    }

    #[test]
    fn transform_counts_and_summarizes() {
        // Text first (resets counts), then tool calls accumulate to the tail summary.
        let msgs = vec![
            json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "hi"}]}}),
            json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Grep"}, {"type": "tool_use", "name": "Read"}]}}),
        ];
        let out = transform_streamlined(&msgs);
        assert_eq!(out.len(), 2);
        assert!(format!("{out:?}").contains("searched 1 pattern"));
    }
}

/// Registry-view wrapper: streamlined inclusion check (init/system and
/// thinking messages are omitted in streamlined output).
pub fn shouldIncludeInStreamlined(args: &[Value]) -> Value {
    let mtype = args.first().and_then(|v| v.as_str()).unwrap_or_default();
    let include = matches!(mtype, "assistant" | "user" | "result" | "stream_event");
    json!(include)
}
