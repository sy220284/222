//! Rust exports derived from original: src/utils/profilerBase.ts
//!
//! Shared profiler infrastructure: one perf timeline for all profilers
//! (process-wide singleton, lazily resolved) and the detailed-report line
//! format. formatMs renders three decimal places; byte sizes go through
//! formatFileSize (format.ts port).

/// `formatMs` — fixed three-decimal rendering.
pub fn format_ms(ms: f64) -> String {
    format!("{:.3}", ms)
}

/// Profiler module names sharing this base (documentation constant).
pub const PROFILER_MODULES: &[&str] = &["startupProfiler", "queryProfiler", "headlessProfiler"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_decimals() {
        assert_eq!(format_ms(1.5), "1.500");
        assert_eq!(format_ms(0.0), "0.000");
        assert_eq!(format_ms(1234.5678), "1234.568");
    }
}
