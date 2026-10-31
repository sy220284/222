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
            294
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
