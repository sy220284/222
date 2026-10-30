//! Source-level command presentation from `src/commands/compact/compact.ts`.
//! The model summary, PreCompact/PostCompact hooks and transcript persistence
//! are provided by Agent::compact_history_custom; this module keeps command
//! feedback and error semantics independent from the model/provider.
#![allow(dead_code)]

pub const NO_MESSAGES: &str = "No messages to compact";
pub const CANCELED: &str = "Compaction canceled.";

/// Exact source `buildDisplayText` content before Ink/chalk applies dimming.
pub fn build_display_text(
    verbose: bool,
    expand_shortcut: &str,
    user_display_message: Option<&str>,
    upgrade_message: Option<&str>,
) -> String {
    let mut fragments = Vec::new();
    if !verbose {
        fragments.push(format!("({expand_shortcut} to see full summary)"));
    }
    if let Some(message) = user_display_message.filter(|s| !s.is_empty()) {
        fragments.push(message.to_owned());
    }
    if let Some(message) = upgrade_message.filter(|s| !s.is_empty()) {
        fragments.push(message.to_owned());
    }
    format!("Compacted {}", fragments.join("\n"))
}

/// Original command preserves cancellation and known summary failures instead
/// of wrapping them with a generic "Error during compaction" message.
pub fn command_error(message: &str, canceled: bool, not_enough: &str, incomplete: &str) -> String {
    if canceled {
        CANCELED.into()
    } else if message == not_enough || message == incomplete {
        message.into()
    } else {
        format!("Error during compaction: {message}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_matches_original_order_and_verbose_toggle() {
        assert_eq!(
            build_display_text(false, "ctrl+o", None, None),
            "Compacted (ctrl+o to see full summary)"
        );
        assert_eq!(build_display_text(true, "ctrl+o", None, None), "Compacted ");
        assert_eq!(
            build_display_text(false, "ctrl+o", Some("Extra"), Some("Upgrade")),
            "Compacted (ctrl+o to see full summary)\nExtra\nUpgrade"
        );
        assert_eq!(
            build_display_text(true, "ctrl+o", Some("Extra"), None),
            "Compacted Extra"
        );
    }
    #[test]
    fn errors_keep_source_precedence() {
        assert_eq!(command_error("wrong", true, "few", "incomplete"), CANCELED);
        assert_eq!(command_error("few", false, "few", "incomplete"), "few");
        assert_eq!(
            command_error("incomplete", false, "few", "incomplete"),
            "incomplete"
        );
        assert_eq!(
            command_error("network", false, "few", "incomplete"),
            "Error during compaction: network"
        );
    }
}
