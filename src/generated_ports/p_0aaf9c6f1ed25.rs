//! Rust exports derived from original: src/ink/components/AlternateScreen.tsx
//!
//! Alt-screen mount decision: DEC 1049 enter/exit around the subtree, mouse
//! tracking (SGR) on by default and torn down with the screen. The write
//! channel and viewport constraint ride the ink runtime; the sequence
//! constants live in the termio dec port.

/// Sequences emitted on mount (enter alt screen, clear, home) — from
/// termio/dec: ENTER_ALT_SCREEN + clear + home.
pub const MOUNT_SEQUENCES: &[&str] = &["\x1b[?1049h", "\x1b[2J", "\x1b[H"];

/// Sequences emitted on unmount (disable mouse + exit alt screen).
pub const UNMOUNT_SEQUENCES: &[&str] = &[
    "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1015l\x1b[?1006l",
    "\x1b[?1049l",
];

/// Mouse tracking default (SGR wheel + click/drag).
pub const MOUSE_TRACKING_DEFAULT: bool = true;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_contracts() {
        assert!(MOUNT_SEQUENCES[0].contains("1049h"));
        assert!(UNMOUNT_SEQUENCES[0].contains("1006l"));
        assert!(UNMOUNT_SEQUENCES[1].contains("1049l"));
        assert!(MOUSE_TRACKING_DEFAULT);
    }
}
