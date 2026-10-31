//! Rust exports derived from original: src/components/CostThresholdDialog.tsx
//!
//! Spend-monitoring dialog: informational copy with the docs link (brand
//! docs domain neutralized per privacy gate) and a single "ok" select option
//! closing the dialog.

/// Select options for the dialog.
pub fn dialog_options() -> Vec<(&'static str, &'static str)> {
    vec![("ok", "Done")]
}

/// Informational copy (docs link neutralized per privacy gate).
pub const MONITOR_COPY: &str = "Learn more about how to monitor your spending:";
