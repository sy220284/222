//! Rust exports derived from original: src/components/hooks/PromptDialog.tsx
//!
//! Selection resolution: a single matching option responds immediately;
//! otherwise the request stays open for interactive selection. Renderer and
//! keybinding wiring belong to the ink host.

/// Whether the request auto-resolves to the only option.
pub fn single_option_autorespond(options: &[String], picked: Option<&str>) -> Option<String> {
    if options.len() == 1 {
        options.first().cloned()
    } else {
        picked.map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autorespond_gate() {
        let one = vec!["only".to_owned()];
        assert_eq!(
            single_option_autorespond(&one, None).as_deref(),
            Some("only")
        );
        let two = vec!["a".to_owned(), "b".to_owned()];
        assert_eq!(
            single_option_autorespond(&two, Some("b")).as_deref(),
            Some("b")
        );
        assert_eq!(single_option_autorespond(&two, None), None);
    }
}
