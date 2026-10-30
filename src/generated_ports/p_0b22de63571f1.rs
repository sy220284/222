//! Rust exports derived from original: src/ink/components/RawAnsi.tsx
//!
//! Fast path for content that is already terminal-ready: bypasses the
//! Ansi→React→Yoga→squash→re-serialize roundtrip and emits a single Yoga
//! leaf (constant-time measure: width × lines) whose joined string goes
//! straight to output.write(). Owned by the ink runtime host; the contract
//! below records the input shape.

/// `Props` — each element must be exactly one terminal row (already wrapped
/// to `width` by the producer) with ANSI escapes inline.
pub struct RawAnsiProps {
    pub lines: Vec<String>,
    pub width: u16,
}

/// The host writes `lines.join("\n")` — output.write already splits on newlines.
pub fn raw_ansi_output(props: &RawAnsiProps) -> String {
    props.lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_lines_in_order() {
        let p = RawAnsiProps {
            lines: vec!["a".into(), "b".into()],
            width: 2,
        };
        assert_eq!(raw_ansi_output(&p), "a\nb");
    }
}
