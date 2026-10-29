//! Rust exports derived from original: src/keybindings/reservedShortcuts.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn NON_REBINDABLE() -> Value {
    serde_json::from_str(r#"[{"key":"ctrl+c","reason":"Cannot be rebound - used for interrupt/exit (hardcoded)","severity":"error"},{"key":"ctrl+d","reason":"Cannot be rebound - used for exit (hardcoded)","severity":"error"},{"key":"ctrl+m","reason":"Cannot be rebound - identical to Enter in terminals (both send CR)","severity":"error"}]"#).expect("Valid source constant")
}

pub fn TERMINAL_RESERVED() -> Value {
    serde_json::from_str(r#"[{"key":"ctrl+z","reason":"Unix process suspend (SIGTSTP)","severity":"warning"},{"key":"ctrl+\\","reason":"Terminal quit signal (SIGQUIT)","severity":"error"}]"#).expect("Valid source constant")
}

pub fn MACOS_RESERVED() -> Value {
    serde_json::from_str(r#"[{"key":"cmd+c","reason":"macOS system copy","severity":"error"},{"key":"cmd+v","reason":"macOS system paste","severity":"error"},{"key":"cmd+x","reason":"macOS system cut","severity":"error"},{"key":"cmd+q","reason":"macOS quit application","severity":"error"},{"key":"cmd+w","reason":"macOS close window/tab","severity":"error"},{"key":"cmd+tab","reason":"macOS app switcher","severity":"error"},{"key":"cmd+space","reason":"macOS Spotlight","severity":"error"}]"#).expect("Valid source constant")
}
