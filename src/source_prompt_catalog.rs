//! Literal prompt evidence from the user-supplied Claude Code 2.1.88 source.
//! Dynamic JS templates remain source text until their parameters are bound.
use serde::Deserialize;
use std::sync::OnceLock;

const SOURCE: &str = include_str!("../docs/original_prompt_literals_2_1_88.jsonl");

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Literal {
    pub line: usize,
    pub quote: String,
    pub raw: String,
    pub dynamic: bool,
}
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Entry {
    pub path: String,
    pub category: String,
    pub source_sha256: String,
    pub source_bytes: usize,
    pub excluded: bool,
    pub literal_candidates: Vec<Literal>,
}
fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        SOURCE
            .lines()
            .map(|line| {
                serde_json::from_str::<Entry>(line)
                    .expect("Built-in original prompt source catalog must be valid")
            })
            .collect()
    })
}

pub fn literal(path: &str, beginning: &str) -> Option<&'static str> {
    entries()
        .iter()
        .find(|entry| entry.path == path && !entry.excluded)?
        .literal_candidates
        .iter()
        .find(|fragment| fragment.raw.starts_with(beginning))
        .map(|fragment| fragment.raw.as_str())
}

/// Original Glob has no template parameters.
pub fn glob_description() -> &'static str {
    literal("src/tools/GlobTool/prompt.ts", "- Fast file pattern matching tool")
        .expect("Glob description must exist in prompt catalog")
}

/// Only wire-name interpolation and JS template escaping differ from source.
pub fn grep_description() -> String {
    let source = literal(
        "src/tools/GrepTool/prompt.ts",
        "A powerful search tool built on ripgrep",
    )
    .expect("Grep description must exist in prompt catalog");
    source
        .replace("\\\u{60}", "\u{60}")
        .replace("\\\\", "\\")
        .replace(concat!("$", "{GREP_TOOL_NAME}"), "grep_files")
        .replace(concat!("$", "{BASH_TOOL_NAME}"), "run_command")
        .replace(concat!("$", "{AGENT_TOOL_NAME}"), "Agent")
}

fn render_original_agent(role: &str, original: &str) -> String {
    // Source TS interpolations are restricted to known, real wire tool names.
    // Do not evaluate arbitrary JavaScript or promote template expressions.
    original
        .replace("\\\u{60}", "\u{60}")
        .replace("Claude Code, Anthropic's official CLI for Claude", "Atlas")
        .replace("Claude Code", "Atlas")
        .replace(concat!("$", "{globGuidance}"), "- Use glob_files for broad file pattern matching")
        .replace(concat!("$", "{grepGuidance}"), "- Use grep_files for searching file contents with regex")
        .replace(concat!("$", "{searchToolsHint}"), "glob_files, grep_files, and read_file")
        .replace(concat!("$", "{FILE_READ_TOOL_NAME}"), "read_file")
        .replace(concat!("$", "{BASH_TOOL_NAME}"), "run_command")
        .replace(concat!("$", "{WEB_FETCH_TOOL_NAME}"), "web_fetch")
        .replace(concat!("$", "{embedded ? ', grep' : ''}"), "")
        .replace(concat!("$", "{hasEmbeddedSearchTools() ? ', grep' : ''}"), "")
        .replace(concat!("$", "{GLOB_TOOL_NAME}"), "glob_files")
        .replace(concat!("$", "{GREP_TOOL_NAME}"), "grep_files")
        .replace(concat!("$", "{AGENT_TOOL_NAME}"), "Agent")
        .replace(concat!("$", "{FILE_EDIT_TOOL_NAME}"), "edit_file")
        .replace(concat!("$", "{FILE_WRITE_TOOL_NAME}"), "write_file")
        .replace(concat!("$", "{NOTEBOOK_EDIT_TOOL_NAME}"), "notebook_edit")
        .replace(concat!("$", "{EXIT_PLAN_MODE_TOOL_NAME}"), "ExitPlanMode")
        .replace("\\\\", "\\")
        .replace("You are a verification specialist.", &format!("You are a verification specialist."))
        .replace(
            "For file searches: search broadly when you don't know where something lives. Use Read ",
            "For file searches: search broadly when you don't know where something lives. Use read_file ",
        )
        .replace("Use Glob, Grep", "Use glob_files, grep_files")
        .replace("when you know the specific file path.", "when you know the specific file path.")
        .replace(
            "VERDICT: PASS",
            if role == "verification" { "VERDICT: PASS" } else { "VERDICT: PASS" },
        )
}

pub fn original_role_instructions(role: &str) -> &'static str {
    fn from_source(path: &str, prefix: &str, title: &str, role: &str) -> String {
        let raw = literal(path, prefix).expect("Original built-in agent prompt exists");
        let content = render_original_agent(role, raw);
        format!("{title}\n{content}")
    }
    static EXPLORE: OnceLock<String> = OnceLock::new();
    static PLAN: OnceLock<String> = OnceLock::new();
    static VERIFY: OnceLock<String> = OnceLock::new();
    static GENERAL: OnceLock<String> = OnceLock::new();
    match role {
        "explore" => EXPLORE.get_or_init(|| {
            from_source(
                "src/tools/AgentTool/built-in/exploreAgent.ts",
                "You are a file search specialist",
                "# Repository exploration agent",
                "explore",
            )
        }),
        "plan" => PLAN.get_or_init(|| {
            from_source(
                "src/tools/AgentTool/built-in/planAgent.ts",
                "You are a software architect",
                "# Planning agent",
                "plan",
            )
        }),
        "verification" => VERIFY.get_or_init(|| {
            from_source(
                "src/tools/AgentTool/built-in/verificationAgent.ts",
                "You are a verification specialist.",
                "# Independent verification agent",
                "verification",
            )
        }),
        _ => GENERAL.get_or_init(|| {
            let path = "src/tools/AgentTool/built-in/generalPurposeAgent.ts";
            let prefix = literal(path, "You are an agent for Claude Code")
                .expect("Source general-purpose agent prefix");
            let guidelines = literal(path, "Your strengths:")
                .expect("Source general-purpose guidelines");
            format!(
                "# Delegated agent\n{} When you complete the task, respond with a concise report covering what was done and any key findings — the caller will relay this to the user, so it only needs the essentials.\n\n{}",
                render_original_agent("general", prefix),
                render_original_agent("general", guidelines)
            )
        }),
    }
    .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_source_paths_are_unique_and_two_remote_features_excluded() {
        let rows = entries();
        assert_eq!(rows.len(), 120);
        let unique = rows.iter().map(|entry| &entry.path).collect::<HashSet<_>>();
        assert_eq!(unique.len(), rows.len());
        assert_eq!(rows.iter().filter(|entry| entry.excluded).count(), 2);
        assert_eq!(
            rows.iter()
                .map(|entry| entry.literal_candidates.len())
                .sum::<usize>(),
            297
        );
        for row in rows {
            assert_eq!(row.source_sha256.len(), 64);
            assert!(row.source_bytes > 0);
            if row.excluded {
                assert!(row.literal_candidates.is_empty());
            }
        }
        assert!(literal("src/utils/teleport.tsx", "").is_none());
        assert!(literal("src/skills/bundled/scheduleRemoteAgents.ts", "").is_none());
    }

    #[test]
    fn actual_runtime_prompts_match_verbatim_source_literals() {
        assert_eq!(glob_description(), crate::search::SOURCE_GLOB_DESCRIPTION);
        assert_eq!(grep_description(), crate::search::SOURCE_GREP_DESCRIPTION);
        assert_eq!(
            literal("src/constants/prompts.ts", "# Executing actions with care"),
            Some(crate::prompt_runtime::SOURCE_ACTIONS)
        );
    }
}
