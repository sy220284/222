//! Rust exports derived from original: src/commands/install-slack-app/install-slack-app.ts
//!
//! The click counter and both result texts are fully equivalent. The
//! marketplace URL contains a vendor-brand path segment; per the privacy
//! gate the URL constant is not carried into Rust (recorded difference) —
//! the browser-launch site rides the browser port when wired.

/// Click-counter decision: increments the persisted install count.
pub fn next_install_count(current: Option<u64>) -> u64 {
    current.unwrap_or(0).saturating_add(1)
}

/// Result texts (original strings).
pub const OPENED_TEXT: &str = "Opening Slack app installation page in browser…";
pub const MANUAL_TEXT: &str = "To install the Slack app, visit: {url}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_starts_at_one_and_grows() {
        assert_eq!(next_install_count(None), 1);
        assert_eq!(next_install_count(Some(41)), 42);
    }
}
