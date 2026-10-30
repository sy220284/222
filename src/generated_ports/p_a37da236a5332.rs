//! Rust exports derived from original: src/utils/computerUse/drainRunLoop.ts
//!
//! macOS CFRunLoop pumping contract: the four @MainActor methods dispatch to
//! DispatchQueue.main and hang under libuv unless the run loop is drained —
//! call sites wrap those calls in drainRunLoop. No native run loop exists in
//! this build (vendor dependency excluded); the wrapper semantics are
//! recorded as the identity pass-through with the original doc preserved.

/// `drainRunLoop` — identity in this build (no native run loop to pump).
pub fn drain_run_loop<T>(value: T) -> T {
    value
}
