//! Rust exports derived from original: src/hooks/useSkillsChange.ts
//!
//! Refresh contract for the commands list across two triggers:
//! 1. Skill file change (watcher): full cache clear + disk re-scan.
//! 2. GrowthBook init/refresh: memo-only clear (isEnabled predicates may
//!    have changed), because getCommands() can run before GB init bakes the
//!    default into the memoized list.

/// Which cache layer a trigger invalidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshKind {
    /// Skill content changed on disk: memo + disk scan.
    Full,
    /// Flag values refreshed: memo-only re-filter.
    MemoOnly,
}

/// `handleChange` decision core: trigger -> invalidation kind (cwd gate
/// included). logError paths ride the log port; onCommandsChange delivery
/// rides the app-state host.
pub fn refresh_kind(cwd: Option<&str>, trigger: RefreshTrigger) -> Option<RefreshKind> {
    if cwd.is_none() {
        return None;
    }
    match trigger {
        RefreshTrigger::SkillFileChange => Some(RefreshKind::Full),
        RefreshTrigger::GrowthBookRefresh => Some(RefreshKind::MemoOnly),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTrigger {
    SkillFileChange,
    GrowthBookRefresh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cwd_gate_and_trigger_mapping() {
        assert_eq!(refresh_kind(None, RefreshTrigger::SkillFileChange), None);
        assert_eq!(
            refresh_kind(Some("/w"), RefreshTrigger::SkillFileChange),
            Some(RefreshKind::Full)
        );
        assert_eq!(
            refresh_kind(Some("/w"), RefreshTrigger::GrowthBookRefresh),
            Some(RefreshKind::MemoOnly)
        );
    }
}
