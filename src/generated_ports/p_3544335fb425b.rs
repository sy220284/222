//! Rust exports derived from original: src/hooks/useExitOnCtrlCDWithKeybindings.ts
//!
//! Composition-only hook: wires useExitOnCtrlCD with useKeybindings. The
//! separation exists to avoid an import cycle (the base hook does not import
//! from the keybindings module directly) — both halves port with the
//! keybindings chain; the re-export of ExitState is preserved here.

pub use super_placeholder::ExitState;

mod super_placeholder {
    /// `ExitState` — idle / first-press / second-press lifecycle of the
    /// double-press exit interaction.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExitState {
        Idle,
        FirstPress,
        SecondPress,
    }
}
