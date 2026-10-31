//! Rust exports derived from original: src/commands/heapdump/heapdump.ts
//!
//! Result wording preserved; `performHeapDump` (heapDumpService) ports with
//! its own file and is recorded as a pending dependency.

/// Failure wording for a heap dump attempt.
pub fn failure_text(error: &str) -> String {
    format!("Failed to create heap dump: {error}")
}

/// Success wording: heap path and diagnostics path, newline-separated.
pub fn success_text(heap_path: &str, diag_path: &str) -> String {
    format!("{heap_path}\n{diag_path}")
}
