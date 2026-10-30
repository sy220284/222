//! Rust exports derived from original: src/commands/extra-usage/extra-usage-noninteractive.ts
//!
//! Non-interactive wrapper wording preserved; `runExtraUsage` (core) ports
//! with the subscription surface and is recorded as a difference.

/// Wording for the browser-opened outcome.
pub fn opened_text(url: &str) -> String {
    format!("Browser opened to manage extra usage. If it didn't open, visit: {url}")
}

/// Wording for the manual-visit outcome.
pub fn manual_visit_text(url: &str) -> String {
    format!("Please visit {url} to manage extra usage.")
}
