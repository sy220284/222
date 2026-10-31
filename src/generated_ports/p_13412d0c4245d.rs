//! Rust exports derived from original: src/components/design-system/StatusIcon.tsx
//!
//! Full equivalence: status → icon glyph and color token mapping, plus the
//! trailing-space option. Color rendering belongs to the ink host.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Error,
    Warning,
    Info,
    Pending,
    Loading,
}

/// Icon glyph per status (original figures characters).
pub fn icon(status: Status) -> &'static str {
    match status {
        Status::Success => "✓",
        Status::Error => "✗",
        Status::Warning => "⚠",
        Status::Info => "ℹ",
        Status::Pending => "○",
        Status::Loading => "…",
    }
}

/// Color token per status (original color names).
pub fn color(status: Status) -> &'static str {
    match status {
        Status::Success => "success",
        Status::Error => "error",
        Status::Warning => "warning",
        Status::Info => "info",
        Status::Pending | Status::Loading => "dim",
    }
}

/// Rendered text (icon plus optional trailing space).
pub fn status_text(status: Status, with_space: bool) -> String {
    if with_space {
        format!("{} ", icon(status))
    } else {
        icon(status).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_and_space() {
        assert_eq!(icon(Status::Success), "✓");
        assert_eq!(color(Status::Pending), "dim");
        assert_eq!(status_text(Status::Error, true), "✗ ");
        assert_eq!(status_text(Status::Error, false), "✗");
    }
}
