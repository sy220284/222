//! Opaque Gemini response parts. REST clients must replay thoughtSignature
//! on its original part, including across tool rounds and session reloads.
//! https://ai.google.dev/gemini-api/docs/generate-content/thought-signatures
use anyhow::{bail, Context, Result};
use serde_json::Value;

pub const PARTS_PREFIX: &str = "__atlas_gemini_parts_v1__:";

pub fn encode(parts: &[Value]) -> Result<String> {
    Ok(format!("{PARTS_PREFIX}{}", serde_json::to_string(parts)?))
}

pub fn decode(state: Option<&str>) -> Result<Option<Vec<Value>>> {
    let Some(raw) = state.and_then(|s| s.strip_prefix(PARTS_PREFIX)) else {
        return Ok(None);
    };
    let parts: Vec<Value> = serde_json::from_str(raw).context("Invalid saved Gemini parts")?;
    if parts.iter().any(|part| !part.is_object()) {
        bail!("Saved Gemini parts must be objects");
    }
    Ok(Some(parts))
}

pub fn visible_text(part: &Value) -> Option<&str> {
    if part.get("thought").and_then(Value::as_bool) == Some(true) {
        None
    } else {
        part.get("text").and_then(Value::as_str)
    }
}
