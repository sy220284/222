//! Native Rust port derived from original: src/components/MemoryUsageIndicator.tsx
//!
//! Ant-only memory indicator: normal status renders null; warning shows
//! formatted heap size in the warning color, critical in error. The
//! /heapdump link is an internal aid; the 10s polling hook is component
//! domain (gated before the hook call so external builds never set it).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

/// Indicator decision: None when normal (or external); Some(color key,
/// text) for warning/critical.
pub fn memory_indicator(
    is_ant: bool,
    heap_used: u64,
    status: &str,
) -> Option<(&'static str, String)> {
    if !is_ant || status == "normal" {
        return None;
    }
    let color = if status == "critical" {
        "error"
    } else {
        "warning"
    };
    let size_mb = heap_used / 1024 / 1024;
    Some((color, format!("{size_mb}MB")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_and_external_render_null() {
        assert!(memory_indicator(true, 100, "normal").is_none());
        assert!(memory_indicator(false, 100, "critical").is_none());
    }

    #[test]
    fn warning_and_critical_colors() {
        let (color, text) = memory_indicator(true, 800 * 1024 * 1024, "warning").unwrap();
        assert_eq!(color, "warning");
        assert_eq!(text, "800MB");
        let (color2, _) = memory_indicator(true, 900 * 1024 * 1024, "critical").unwrap();
        assert_eq!(color2, "error");
    }
}
