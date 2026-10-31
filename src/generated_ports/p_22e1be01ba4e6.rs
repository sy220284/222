//! Rust exports derived from original: src/components/messages/teamMemSaved.ts
//!
//! Full equivalence: team-memory segment for the memory-saved UI plus the
//! count; null when zero. Plain function (no React) so memoization cannot
//! hoist the teamCount access — original note kept.

/// `teamMemSavedPart`.
pub fn team_mem_saved_part(team_count: Option<u64>) -> Option<(String, u64)> {
    let count = team_count.unwrap_or(0);
    if count == 0 {
        return None;
    }
    let noun = if count == 1 { "memory" } else { "memories" };
    Some((format!("{count} team {noun}"), count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_none_and_pluralization() {
        assert_eq!(team_mem_saved_part(None), None);
        assert_eq!(team_mem_saved_part(Some(0)), None);
        assert_eq!(
            team_mem_saved_part(Some(1)),
            Some(("1 team memory".into(), 1))
        );
        assert_eq!(
            team_mem_saved_part(Some(3)),
            Some(("3 team memories".into(), 3))
        );
    }
}
