//! Multi-model equivalent of the original interactive project /init workflow.
//! Discovery stays read-only; the model uses the ordinary permission-gated
//! tools to propose and persist the user's selected artifacts.
use anyhow::{Context, Result};
use std::{fs, path::Path};

const ROOT_MARKERS: &[&str] = &[
    "AGENTS.md",
    "AGENTS.local.md",
    "README.md",
    "README",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "bun.lock",
    "bun.lockb",
    "yarn.lock",
    "pyproject.toml",
    "uv.lock",
    "requirements.txt",
    "go.mod",
    "go.work",
    "pom.xml",
    "build.gradle",
    "Makefile",
    "justfile",
    ".github/copilot-instructions.md",
    ".cursorrules",
    ".cursor/rules",
    ".windsurfrules",
    ".clinerules",
    ".github/workflows",
    ".gitignore",
    ".git/info/exclude",
    ".mcp.json",
    ".atlas/skills",
    ".atlas/hooks",
    ".atlas/rules",
    "CLAUDE.md",
    "CLAUDE.local.md",
];

/// Reports actual paths only; no repository file content or environment
/// variables are embedded into a generated prompt before tool permissions.
pub fn markers(root: &Path) -> Result<Vec<String>> {
    let root = root.canonicalize().context("Workspace unavailable")?;
    let mut found = Vec::new();
    for relative in ROOT_MARKERS {
        let path = root.join(relative);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() {
                found.push(format!(
                    "{relative} (symbolic link: inspect target before use)"
                ));
            } else if meta.is_dir() {
                found.push(format!("{relative}/"));
            } else if meta.is_file() {
                found.push(relative.to_string());
            }
        }
    }
    found.sort();
    Ok(found)
}

pub fn prompt(root: &Path) -> Result<String> {
    let found = markers(root)?;
    let inventory = if found.is_empty() {
        "No conventional root manifests were found; inspect the tree.".to_owned()
    } else {
        found.join("\n")
    };
    Ok(format!(
        r#"Initialize this repository's instructions using the provider-neutral interactive equivalent of the original /init workflow. Work entirely in the active workspace, preserve existing content, and use real model tools to inspect files and implement only what the user approves.

Detected root landmarks (names only, not their contents):
{inventory}

Phase 1 — Ask user preferences via AskUserQuestion *before changing files*:
1. Which instruction documents? Options Project AGENTS.md (tracked), Personal AGENTS.local.md (private), Both.
2. Also create optional workflows? Options Skills + hooks, Skills only, Hooks only, Neither.
Do not assume the answer, and do not write both unless selected.

Phase 2 — Inspect actual sources with file-reading tools. Read the relevant root files, package/build/test commands, CI, existing AGENTS.md, user rules (.cursor/.github/copilot), configured MCP servers and workflows. Explore architecture by subagent if needed. Do not exfiltrate credentials, inspect private files without permission, or report invented commands or settings. If files were marked as symbolic links, verify their location before use. Keep the final instructions concise, focusing on project-specific constraints rather than general platitudes.

Phase 3 — Ask only the remaining genuinely unknown details, via AskUserQuestion (e.g., non-obvious team conventions, personal preferences). Synthesize a concrete proposed AGENTS.md/AGENTS.local.md update plus optional hooks/skills. Show the proposal in the AskUserQuestion option previews; allow removal of individual items. Respect the initial skills/hooks selection.

Phase 4 — After confirmation, use ordinary permission-checked edit/write tools to apply the proposed changes. Project document AGENTS.md remains tracked. Personal AGENTS.local.md must be excluded from version control; confirm existing ignore rules and append an exact non-broad ignore pattern only when needed. If AGENTS.md exists, read it and propose incremental improvements, never wipe it or blindly replace. Don't edit CLAUDE.md or other vendor-exclusive files; they are useful references only.

Phase 5 — For selected skills, use .atlas/skills/<skill-name>/SKILL.md with concrete instructions and workflow entry points. For hooks, use the provider-neutral local hook config already supported by this runtime; ask before enabling any command with side effects. Do not run unexpected installers. Omit optional artifacts that the user did not select.

Phase 6 — Validate that each file exists, is parseable when structured, and that its concrete listed commands follow from the repository's actual manifests. If feasible run agreed tests after requesting permission, then summarize exact changes and known unresolved questions. Never treat a tool call's success text as verification. Answer in the user's language.

This is a user-invoked project initialization request. Execute the entire workflow now, not just give suggestions."#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_real_landmarks_without_reading_secrets_or_following_links() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("README.md"), "token=DO_NOT_COPY").unwrap();
        fs::write(root.path().join("Cargo.toml"), "[package]").unwrap();
        let found = markers(root.path()).unwrap();
        assert!(found.contains(&"README.md".to_owned()));
        assert!(found.contains(&"Cargo.toml".to_owned()));
        let prompt = prompt(root.path()).unwrap();
        assert!(!prompt.contains("DO_NOT_COPY"));
        assert!(prompt.contains("AGENTS.md"));
        assert!(prompt.contains("AskUserQuestion"));
        assert!(prompt.contains("Skills only"));
        assert!(!prompt.contains("company login"));
    }
    #[cfg(unix)]
    #[test]
    fn symlink_source_is_not_silently_followed() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("README.md"), "PRIVATE").unwrap();
        symlink(
            outside.path().join("README.md"),
            root.path().join("README.md"),
        )
        .unwrap();
        let prompt = prompt(root.path()).unwrap();
        assert!(prompt.contains("README.md (symbolic link"));
        assert!(!prompt.contains("PRIVATE"));
    }
}
