//! Rust exports derived from original: src/components/permissions/WorkerBadge.tsx
//!
//! Colored badge identifying which swarm worker requests a permission:
//! BLACK_CIRCLE glyph + worker name in the worker color (via toInkColor).
//! JSX and the figures constant belong to the ink host; the color mapping
//! helper is already available through the ink utils port.

/// Badge text construction: glyph + space + worker name.
pub fn worker_badge_text(name: &str, black_circle: &str) -> String {
    format!("{black_circle} {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_text_format() {
        assert_eq!(worker_badge_text("worker-1", "●"), "● worker-1");
    }
}
