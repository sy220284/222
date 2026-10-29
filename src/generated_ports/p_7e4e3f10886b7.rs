//! Rust exports derived from original: src/constants/figures.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn BULLET_OPERATOR() -> Value {
    serde_json::from_str(r#""∙""#).expect("Valid source constant")
}

pub fn TEARDROP_ASTERISK() -> Value {
    serde_json::from_str(r#""✻""#).expect("Valid source constant")
}

pub fn UP_ARROW() -> Value {
    serde_json::from_str(r#""↑""#).expect("Valid source constant")
}

pub fn DOWN_ARROW() -> Value {
    serde_json::from_str(r#""↓""#).expect("Valid source constant")
}

pub fn LIGHTNING_BOLT() -> Value {
    serde_json::from_str(r#""↯""#).expect("Valid source constant")
}

pub fn EFFORT_LOW() -> Value {
    serde_json::from_str(r#""○""#).expect("Valid source constant")
}

pub fn EFFORT_MEDIUM() -> Value {
    serde_json::from_str(r#""◐""#).expect("Valid source constant")
}

pub fn EFFORT_HIGH() -> Value {
    serde_json::from_str(r#""●""#).expect("Valid source constant")
}

pub fn EFFORT_MAX() -> Value {
    serde_json::from_str(r#""◉""#).expect("Valid source constant")
}

pub fn PLAY_ICON() -> Value {
    serde_json::from_str(r#""▶""#).expect("Valid source constant")
}

pub fn PAUSE_ICON() -> Value {
    serde_json::from_str(r#""⏸""#).expect("Valid source constant")
}

pub fn REFRESH_ARROW() -> Value {
    serde_json::from_str(r#""↻""#).expect("Valid source constant")
}

pub fn CHANNEL_ARROW() -> Value {
    serde_json::from_str(r#""←""#).expect("Valid source constant")
}

pub fn INJECTED_ARROW() -> Value {
    serde_json::from_str(r#""→""#).expect("Valid source constant")
}

pub fn FORK_GLYPH() -> Value {
    serde_json::from_str(r#""⑂""#).expect("Valid source constant")
}

pub fn DIAMOND_OPEN() -> Value {
    serde_json::from_str(r#""◇""#).expect("Valid source constant")
}

pub fn DIAMOND_FILLED() -> Value {
    serde_json::from_str(r#""◆""#).expect("Valid source constant")
}

pub fn REFERENCE_MARK() -> Value {
    serde_json::from_str(r#""※""#).expect("Valid source constant")
}

pub fn FLAG_ICON() -> Value {
    serde_json::from_str(r#""⚑""#).expect("Valid source constant")
}

pub fn BLOCKQUOTE_BAR() -> Value {
    serde_json::from_str(r#""▎""#).expect("Valid source constant")
}

pub fn HEAVY_HORIZONTAL() -> Value {
    serde_json::from_str(r#""━""#).expect("Valid source constant")
}

pub fn BRIDGE_SPINNER_FRAMES() -> Value {
    serde_json::from_str(r#"["·|·","·/·","·—·","·\\·"]"#).expect("Valid source constant")
}

pub fn BRIDGE_READY_INDICATOR() -> Value {
    serde_json::from_str(r#""·✔︎·""#).expect("Valid source constant")
}

pub fn BRIDGE_FAILED_INDICATOR() -> Value {
    serde_json::from_str(r#""×""#).expect("Valid source constant")
}
