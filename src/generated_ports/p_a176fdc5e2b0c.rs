//! Rust exports derived from original: src/voice/voiceModeEnabled.ts
//!
//! Kill-switch shape preserved: voice visibility requires the compile-time
//! VOICE_MODE gate, and the vendor kill flag defaults to "not killed" so a
//! missing/stale cache reads as enabled. This crate keeps the gate closed
//! (the original's external-build behavior), so the visible check is false
//! exactly like the original with the gate off; the auth-only half depends
//! on the vendor OAuth surface (excluded) and stays recorded here.

/// `isVoiceGrowthBookEnabled` — with the VOICE_MODE gate closed this is
/// false, byte-identical to the original ternary's else branch.
pub fn is_voice_growth_book_enabled() -> bool {
    false
}
