//! Rust exports derived from original: original UI error-boundary source module (vendor telemetry removed)
//!
//! Error-boundary contract: on a child render error the boundary flips
//! hasError and renders null (a blank subtree). Rust's terminal runtime has
//! no React component tree; the state machine is preserved for the UI host.

/// Boundary state machine from the original class component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiErrorBoundaryState {
    pub has_error: bool,
}

impl UiErrorBoundaryState {
    /// `getDerivedStateFromError` — any child error marks the boundary.
    pub fn from_error() -> Self {
        Self { has_error: true }
    }
    /// `render` — a tripped boundary renders nothing.
    pub fn renders_null(&self) -> bool {
        self.has_error
    }
}
