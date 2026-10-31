//! Rust exports derived from original: src/components/messages/UserImageMessage.tsx
//!
//! Full equivalence: the attachment label is `[Image #N]` with an id and
//! `[Image]` without; the stored-path + hyperlinks combination decides
//! whether the label renders as a link. Link/hyperlink support rides the
//! ink ports.

/// `label` construction.
pub fn image_label(image_id: Option<u64>) -> String {
    match image_id {
        Some(id) => format!("[Image #{id}]"),
        None => "[Image]".to_owned(),
    }
}

/// Whether the label renders as a hyperlink (stored path + terminal
/// hyperlinks support).
pub fn renders_as_link(
    image_id: Option<u64>,
    stored_path: Option<&str>,
    supports_hyperlinks: bool,
) -> bool {
    image_id.is_some() && stored_path.is_some() && supports_hyperlinks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_link_gate() {
        assert_eq!(image_label(Some(3)), "[Image #3]");
        assert_eq!(image_label(None), "[Image]");
        assert!(renders_as_link(Some(1), Some("/tmp/a.png"), true));
        assert!(!renders_as_link(Some(1), Some("/tmp/a.png"), false));
        assert!(!renders_as_link(None, Some("/tmp/a.png"), true));
    }
}
