//! Rust exports derived from original: src/ink/components/Newline.tsx
//!
//! Fully equivalent: renders one or more newline characters, defaulting to
//! one when no count is given.

/// Rendered newline payload for `count` (defaults to 1).
pub fn newline_text(count: Option<usize>) -> String {
    "\n".repeat(count.unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_one_and_repeats() {
        assert_eq!(newline_text(None), "\n");
        assert_eq!(newline_text(Some(3)), "\n\n\n");
    }
}
