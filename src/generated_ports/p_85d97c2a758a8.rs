//! Rust exports derived from original: src/components/teams/TeamStatus.tsx
//!
//! Footer status decision: teammate count (non-error members) plus hint
//! visibility gates — "teams selected" shows the count line, "hint" shows
//! the shortcut hint. Member filtering and JSX belong to the ink host.

/// Which footer lines render for this state.
pub fn team_status_lines(teams_selected: bool, show_hint: bool) -> &'static str {
    match (teams_selected, show_hint) {
        (true, true) => "count+hint",
        (true, false) => "count",
        (false, true) => "hint",
        (false, false) => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_matrix() {
        assert_eq!(team_status_lines(true, true), "count+hint");
        assert_eq!(team_status_lines(false, false), "none");
    }
}
