//! Rust exports derived from original: src/components/BypassPermissionsModeDialog.tsx
//!
//! Dialog decision core: accepting writes the settings flag and continues
//! (onAccept), the cancel item performs a graceful synchronous shutdown and
//! never returns to the dialog. Vendor analytics events are excluded. The
//! Select/Dialog rendering belongs to the ink host.

pub enum BypassDialogChoice {
    Accept,
    CancelShutdown,
}

/// Outcome of a dialog choice: whether the settings flag should be written.
pub fn apply_choice(choice: &BypassDialogChoice) -> bool {
    matches!(choice, BypassDialogChoice::Accept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_matrix() {
        assert!(apply_choice(&BypassDialogChoice::Accept));
        assert!(!apply_choice(&BypassDialogChoice::CancelShutdown));
    }
}
