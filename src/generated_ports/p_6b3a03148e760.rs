//! Rust exports derived from original: src/hooks/notifs/useInstallMessages.tsx
//!
//! The message-priority mapping is fully equivalent and tested: errors and
//! user-action-required items rank high, path/alias rank medium, everything
//! else low; the color mirrors the type. checkInstall rides the native
//! installer port; the startup-notification host lives in the TUI layer.

/// Notification priority for an install message.
pub fn install_message_priority(message_type: &str, user_action_required: bool) -> &'static str {
    if message_type == "error" || user_action_required {
        "high"
    } else if message_type == "path" || message_type == "alias" {
        "medium"
    } else {
        "low"
    }
}

/// Notification color for an install message.
pub fn install_message_color(message_type: &str) -> &'static str {
    if message_type == "error" {
        "error"
    } else {
        "warning"
    }
}

/// `key` construction: install-message-{index}-{type}.
pub fn install_message_key(index: usize, message_type: &str) -> String {
    format!("install-message-{index}-{message_type}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_mapping_matches_original() {
        assert_eq!(install_message_priority("error", false), "high");
        assert_eq!(install_message_priority("ok", true), "high");
        assert_eq!(install_message_priority("path", false), "medium");
        assert_eq!(install_message_priority("alias", false), "medium");
        assert_eq!(install_message_priority("ok", false), "low");
        assert_eq!(install_message_color("error"), "error");
        assert_eq!(install_message_color("path"), "warning");
        assert_eq!(install_message_key(2, "path"), "install-message-2-path");
    }
}
