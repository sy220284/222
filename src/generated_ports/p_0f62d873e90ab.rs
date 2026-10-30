//! Rust exports derived from original: src/components/EffortIndicator.ts
//!
//! Level-to-figure mapping and notification wording, equivalent to the
//! original switch over EffortLevel with the constants/figures symbols.

/// `effortLevelToSymbol` — figures from the original constants/figures.
pub fn effort_level_to_symbol(level: &str) -> &'static str {
    match level {
        "low" => "\u{25D0}",
        "medium" => "\u{25D1}",
        "high" => "\u{25D2}",
        "max" => "\u{25D3}",
        _ => "\u{25CB}",
    }
}

/// `getEffortNotificationText` — undefined when the model does not support
/// effort (decision delegated to the effort utils port).
pub fn effort_notification_text(level: Option<&str>) -> Option<String> {
    let level = level?;
    Some(format!(
        "{} {level} \u{00B7} /effort",
        effort_level_to_symbol(level)
    ))
}
