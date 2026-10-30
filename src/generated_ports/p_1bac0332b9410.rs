//! Rust exports derived from original: src/components/wizard/WizardDialogLayout.tsx
//!
//! Layout decision core: dialog color defaults to "suggestion"; footer text
//! defaults to the navigation footer (which swaps to the exit prompt while a
//! Ctrl-CD exit is pending). Dialog chrome and children belong to the ink
//! host.

/// Effective dialog color.
pub fn dialog_color(override_color: Option<&str>) -> &str {
    override_color.unwrap_or("suggestion")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_defaults_to_suggestion() {
        assert_eq!(dialog_color(None), "suggestion");
        assert_eq!(dialog_color(Some("warning")), "warning");
    }
}
