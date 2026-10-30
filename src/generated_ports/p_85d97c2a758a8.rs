//! Rust exports derived from original: src/components/teams/TeamStatus.tsx
//!
//! Status line construction: non-empty segments joined with the separator;
//! None when nothing to show. Owned by the ink host for rendering.

/// Join non-empty status segments; None when all are empty.
pub fn status_line(segments: &[Option<String>]) -> Option<String> {
    let parts: Vec<&str> = segments
        .iter()
        .filter_map(|s| s.as_deref())
        .filter(|s| !s.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_and_omission() {
        let segs = vec![Some("3 tasks".into()), None, Some("1 done".into())];
        assert_eq!(status_line(&segs).as_deref(), Some("3 tasks · 1 done"));
        assert_eq!(status_line(&[None, None]), None);
    }
}
