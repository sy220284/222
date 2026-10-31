//! Rust exports derived from original:
//! src/components/shell/ExpandShellOutputContext.tsx
//!
//! Boolean context: when true, shell output renders in full (not truncated)
//! — used to auto-expand the most recent user `!` command output. Same
//! pattern as MessageResponseContext / SubAgentContext. Provider always
//! supplies true; default is false.

/// Default when no provider is mounted.
pub const DEFAULT_EXPAND: bool = false;

/// Provider-supplied value.
pub const EXPANDED: bool = true;
