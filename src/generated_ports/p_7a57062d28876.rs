//! Rust exports derived from original: src/ink/components/StdinContext.ts
//!
//! Stdin surface contract: raw stream, setRawMode (no-op when unsupported),
//! isRawModeSupported fallback flag, internal exitOnCtrlC flag, the internal
//! event emitter and the terminal querier (DECRQM, OSC 11). Owned by the ink
//! runtime host.

/// Surface identity — components read these five capabilities.
pub struct StdinSurface {
    pub set_raw_mode_supported: bool,
    pub internal_exit_on_ctrl_c: bool,
}
