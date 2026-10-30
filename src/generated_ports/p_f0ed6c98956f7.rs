//! Rust exports derived from original: src/replLauncher.tsx
//!
//! Launch contract: lazily imports App + REPL and hands the composed element
//! tree to the injected renderAndRun. The ink render host owns Root; props
//! are the stats store, FPS metrics getter and the initial AppState.

pub const LAZY_APP_MODULE: &str = "./components/App.js";
pub const LAZY_REPL_MODULE: &str = "./screens/REPL.js";
