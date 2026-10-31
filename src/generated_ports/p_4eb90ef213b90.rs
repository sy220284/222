//! Rust exports derived from original: src/utils/computerUse/swiftLoader.ts
//!
//! The original lazily loads the macOS-only native computer-use module and
//! caches it. The Rust runtime carries no native Swift bridge; the platform
//! guard and cached-singleton semantics are preserved, and callers on
//! unsupported platforms receive the original's error surface.

/// Cached handle placeholder for the native module. The four @MainActor
/// methods dispatch to DispatchQueue.main and require a pumped CFRunLoop —
/// call sites must wrap them in drainRunLoop() semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputerUseApi;

static CACHED: std::sync::OnceLock<ComputerUseApi> = std::sync::OnceLock::new();

/// `requireComputerUseSwift` — throws on non-darwin platforms, caches the
/// loaded module otherwise.
pub fn require_computer_use_swift() -> anyhow::Result<ComputerUseApi> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!("@ant/computer-use-swift is macOS-only")
    }
    Ok(*CACHED.get_or_init(|| ComputerUseApi))
}
