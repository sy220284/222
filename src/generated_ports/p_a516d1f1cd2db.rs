//! Rust exports derived from original: src/components/messages/CompactBoundaryMessage.tsx
//!
//! Full equivalence of the text core: "✻ Conversation compacted ({shortcut}
//! for history)" with the shortcut resolved from keybindings
//! (app:toggleTranscript / Global / ctrl+o default). Box marginY=1 and dim
//! styling ride the ink host.

pub fn compact_boundary_text(history_shortcut: &str) -> String {
    format!("✻ Conversation compacted ({history_shortcut} for history)")
}

/// Keybinding triple consumed by useShortcutDisplay in the original.
pub const SHORTCUT_ACTION: &str = "app:toggleTranscript";
pub const SHORTCUT_CONTEXT: &str = "Global";
pub const SHORTCUT_DEFAULT: &str = "ctrl+o";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_matches_original_layout() {
        assert_eq!(
            compact_boundary_text("ctrl+o"),
            "✻ Conversation compacted (ctrl+o for history)"
        );
    }
}
