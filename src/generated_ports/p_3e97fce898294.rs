//! Rust exports derived from original: src/context/fpsMetrics.tsx
//!
//! React context that stores an FpsMetrics getter and never re-renders
//! consumers on value change (the getter identity is stable). The Rust
//! terminal runtime hosts contexts; the port records the contract.

/// The context value contract: a getter for optional FpsMetrics.
pub type FpsMetricsGetter = Box<dyn Fn() -> Option<serde_json::Value> + Send + Sync>;

/// Provider props contract (getter + children) — no reactive semantics.
pub struct FpsMetricsProviderContract {
    pub get_fps_metrics: FpsMetricsGetter,
}
