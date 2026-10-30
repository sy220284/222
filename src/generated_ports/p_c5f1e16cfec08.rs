//! Rust exports derived from original: src/commands/stickers/stickers.ts
//!
//! Result wording is preserved. The sticker page URL is brand merchandise
//! (vendor branding is excluded per the port's privacy policy), so the
//! concrete URL is intentionally not carried into the Rust tree; the browser
//! open itself rides the existing browser_launcher when the command host
//! lands.

/// Result wording for a successful browser open.
pub const OPENED_TEXT: &str = "Opening sticker page in browser\u{2026}";

/// Result wording when no browser could be opened (URL supplied at runtime).
pub fn failed_text(url: &str) -> String {
    format!("Failed to open browser. Visit: {url}")
}
