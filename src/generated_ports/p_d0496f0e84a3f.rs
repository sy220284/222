//! Rust exports derived from original: src/components/messages/AssistantThinkingMessage.tsx
//!
//! Visibility decision core: past thinking in transcript mode hides
//! entirely; streaming thinking renders with margin; the expand hint only
//! appears when thinking text is present. Markdown body belongs to the ink
//! host.

pub struct ThinkingView {
    pub add_margin: bool,
    pub show_expand_hint: bool,
}

/// `AssistantThinkingMessage` render decision.
pub fn thinking_view(
    hide_in_transcript: bool,
    is_transcript_mode: bool,
    thinking_text: &str,
) -> Option<ThinkingView> {
    if hide_in_transcript {
        return None;
    }
    Some(ThinkingView {
        add_margin: !is_transcript_mode,
        show_expand_hint: !thinking_text.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_matrix() {
        assert!(thinking_view(true, true, "x").is_none());
        let v = thinking_view(false, false, "x").unwrap();
        assert!(v.add_margin && v.show_expand_hint);
        let v = thinking_view(false, true, "x").unwrap();
        assert!(!v.add_margin && v.show_expand_hint);
        let v = thinking_view(false, false, "").unwrap();
        assert!(!v.show_expand_hint);
    }
}
