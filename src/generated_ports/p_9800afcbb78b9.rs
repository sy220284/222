//! Rust exports derived from original: src/plugins/bundled/index.ts
//!
//! Built-in plugin initialization scaffolding. The original registers no
//! built-in plugins yet; the entry point is preserved verbatim so bundled
//! features that should be user-toggleable have the same registration site.

/// `initBuiltinPlugins` — called during CLI startup. No built-in plugins are
/// registered yet; this is the scaffolding for migrating bundled skills that
/// should be user-toggleable.
pub fn init_builtin_plugins() {}
