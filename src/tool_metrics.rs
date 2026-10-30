//! Code-edit counters follow the original structured patch line accounting.
//! Each inserted/deleted diff line increments the session-level total.
use similar::{ChangeTag, TextDiff};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct ToolMetrics {
    lines_added: AtomicU64,
    lines_removed: AtomicU64,
}
impl ToolMetrics {
    pub fn with_totals(added: u64, removed: u64) -> Self {
        Self {
            lines_added: AtomicU64::new(added),
            lines_removed: AtomicU64::new(removed),
        }
    }
    pub fn record_change(&self, before: Option<&str>, after: &str) {
        let (added, removed) = count_lines_changed(before, after);
        self.lines_added.fetch_add(added, Ordering::Relaxed);
        self.lines_removed.fetch_add(removed, Ordering::Relaxed);
    }
    pub fn add_counts(&self, added: u64, removed: u64) {
        self.lines_added.fetch_add(added, Ordering::Relaxed);
        self.lines_removed.fetch_add(removed, Ordering::Relaxed);
    }
    pub fn counts(&self) -> (u64, u64) {
        (
            self.lines_added.load(Ordering::Relaxed),
            self.lines_removed.load(Ordering::Relaxed),
        )
    }
}
pub fn count_lines_changed(before: Option<&str>, after: &str) -> (u64, u64) {
    if before.is_none() {
        if after.is_empty() {
            return (0, 0);
        }
        // Original new-file path uses split(/\r?\n/).length, including a final
        // empty segment when the file ends with a newline.
        return (after.split('\n').count() as u64, 0);
    }
    let diff = TextDiff::from_lines(before.unwrap_or(""), after);
    let mut added = 0u64;
    let mut removed = 0u64;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    (added, removed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_new_files_updates_removals_and_unchanged_text() {
        assert_eq!(count_lines_changed(None, ""), (0, 0));
        assert_eq!(count_lines_changed(None, "one"), (1, 0));
        assert_eq!(count_lines_changed(None, "one\n"), (2, 0));
        assert_eq!(count_lines_changed(Some("a\nb\n"), "a\nc\n"), (1, 1));
        assert_eq!(count_lines_changed(Some("a\nb\n"), "a\n"), (0, 1));
        assert_eq!(count_lines_changed(Some("a\n"), "a\n"), (0, 0));
        let state = ToolMetrics::with_totals(5, 7);
        state.record_change(Some("a\n"), "a\nb\n");
        assert_eq!(state.counts(), (6, 7));
    }
}
