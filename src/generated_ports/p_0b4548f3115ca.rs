//! Native Rust port derived from original: src/utils/teamMemoryOps.ts
//!
//! Team-memory activity summarization and file classification. The
//! original's `isTeamMemFile` gates on the full `memdir/teamMemPaths`
//! port (sanitize/realpath/feature flags — 292 lines, pending its own
//! migration); this port reproduces the **pure path-shape predicate** and
//! the complete display-summary builder. The feature-flag gate
//! (`experiment-flag (original name elided for privacy scan)` experiment) maps to a process-lifetime switch.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub static TEAM_MEMORY_ENABLED: AtomicBool = AtomicBool::new(true);

/// Original gate: auto-memory enabled AND `experiment-flag (original name elided for privacy scan)` feature
/// on. Auto-memory enablement is an env/config port concern; the flag
/// default mirrors an enabled production experiment rollout.
pub fn is_team_memory_enabled() -> bool {
    TEAM_MEMORY_ENABLED.load(Ordering::Relaxed)
}

/// Pure path-shape check: the file lives inside the team memory
/// directory (`<base>/team-memory`). Security-critical traversal and
/// symlink checks live in the teamMemPaths port and are re-applied by
/// callers before any write.
pub fn is_team_mem_path(file_path: &Path, memory_base: &Path) -> bool {
    let team_dir: PathBuf = memory_base.join("team-memory");
    file_path.starts_with(&team_dir)
}

/// Original `isTeamMemFile`: enabled gate + path-shape check.
pub fn is_team_mem_file(file_path: &Path, memory_base: &Path) -> bool {
    is_team_memory_enabled() && is_team_mem_path(file_path, memory_base)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeamMemoryCounts {
    pub team_memory_read_count: u64,
    pub team_memory_search_count: u64,
    pub team_memory_write_count: u64,
}

/// Builds the activity summary shown in the tool display, matching the
/// original's verb forms exactly:
/// - active: "Recalling"/"recalling", "Searching"/"searching",
///   "Writing"/"writing" (capitalized only when no earlier part exists)
/// - completed: "Recalled"/"recalled", "Searched"/"searched",
///   "Wrote"/"wrote"
/// - singular "memory" vs plural "memories" for reads/writes
pub fn build_team_memory_summary(counts: TeamMemoryCounts, is_active: bool) -> String {
    let mut parts: Vec<String> = Vec::new();

    if counts.team_memory_read_count > 0 {
        let verb = if is_active {
            if parts.is_empty() {
                "Recalling"
            } else {
                "recalling"
            }
        } else if parts.is_empty() {
            "Recalled"
        } else {
            "recalled"
        };
        let noun = if counts.team_memory_read_count == 1 {
            "memory"
        } else {
            "memories"
        };
        parts.push(format!(
            "{verb} {} team {noun}",
            counts.team_memory_read_count
        ));
    }

    if counts.team_memory_search_count > 0 {
        let verb = if is_active {
            if parts.is_empty() {
                "Searching"
            } else {
                "searching"
            }
        } else if parts.is_empty() {
            "Searched"
        } else {
            "searched"
        };
        parts.push(format!("{verb} team memories"));
    }

    if counts.team_memory_write_count > 0 {
        let verb = if is_active {
            if parts.is_empty() {
                "Writing"
            } else {
                "writing"
            }
        } else if parts.is_empty() {
            "Wrote"
        } else {
            "wrote"
        };
        let noun = if counts.team_memory_write_count == 1 {
            "memory"
        } else {
            "memories"
        };
        parts.push(format!(
            "{verb} {} team {noun}",
            counts.team_memory_write_count
        ));
    }

    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_verb_forms_match_original() {
        assert_eq!(
            build_team_memory_summary(
                TeamMemoryCounts {
                    team_memory_read_count: 2,
                    ..Default::default()
                },
                true
            ),
            "Recalling 2 team memories"
        );
        assert_eq!(
            build_team_memory_summary(
                TeamMemoryCounts {
                    team_memory_read_count: 1,
                    ..Default::default()
                },
                false
            ),
            "Recalled 1 team memory"
        );
    }

    #[test]
    fn secondary_parts_use_lowercase_verb() {
        let counts = TeamMemoryCounts {
            team_memory_read_count: 1,
            team_memory_search_count: 1,
            team_memory_write_count: 1,
        };
        assert_eq!(
            build_team_memory_summary(counts, false),
            "Recalled 1 team memory, searched team memories, wrote 1 team memory"
        );
        assert_eq!(
            build_team_memory_summary(counts, true),
            "Recalling 1 team memory, searching team memories, writing 1 team memory"
        );
    }

    #[test]
    fn empty_counts_yield_empty_summary() {
        assert_eq!(
            build_team_memory_summary(TeamMemoryCounts::default(), true),
            ""
        );
    }

    #[test]
    fn path_shape_check_scopes_to_team_memory_dir() {
        let base = Path::new("/home/user/.atlas/memory");
        assert!(is_team_mem_path(
            Path::new("/home/user/.atlas/memory/team-memory/note.md"),
            base
        ));
        assert!(!is_team_mem_path(
            Path::new("/home/user/.atlas/memory/auto/x.md"),
            base
        ));
        assert!(!is_team_mem_path(
            Path::new("/home/user/.atlas/memory/team-memory-evil/x.md"),
            base
        ));
    }
}
