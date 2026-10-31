//! Rust exports derived from original: src/components/wizard/WizardNavigationFooter.tsx
//!
//! Footer decision: when the Ctrl-CD exit state is pending, instructions are
//! replaced by "Press {key} again to exit"; otherwise the default hint list
//! (↑↓ navigate / Enter select / Esc go back) renders. Layout margins
//! (marginLeft 3 / marginTop 1) and dim color belong to the ink host.

/// Text to render in the footer.
pub fn footer_text(pending: bool, key_name: &str, instructions: &str) -> String {
    if pending {
        format!("Press {key_name} again to exit")
    } else {
        instructions.to_owned()
    }
}

/// Default instructions (Byline + three shortcut hints).
pub const DEFAULT_INSTRUCTIONS: &str = "↑↓ navigate · Enter select · Esc to go back";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_replaces_instructions() {
        assert_eq!(
            footer_text(true, "ctrl+c", "hint"),
            "Press ctrl+c again to exit"
        );
        assert_eq!(footer_text(false, "ctrl+c", "hint"), "hint");
    }
}
