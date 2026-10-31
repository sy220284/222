//! Rust exports derived from original: src/utils/computerUse/gates.ts
//!
//! Full equivalence at the defaults layer: partial remote config spreads over
//! defaults so `{"enabled": true}` alone inherits the rest — the generic in
//! the original is a type assertion, not a validator, so missing fields must
//! surface as defaults, never undefined. Remote fetch rides the analytics
//! port (vendor exclusion recorded).

use serde_json::Value;

/// `DEFAULTS` — ChicagoConfig defaults (verbatim values).
pub fn defaults() -> Value {
    serde_json::json!({
        "enabled": false,
        "pixelValidation": false,
        "clipboardPasteMultiline": true,
        "mouseAnimation": true,
        "hideBeforeAction": true,
        "autoTargetDisplay": true,
        "clipboardGuard": true,
        "coordinateMode": "pixels"
    })
}

/// `readConfig` merge semantics: defaults under, partial over.
pub fn spread_over_defaults(partial: &Value) -> Value {
    let mut out = defaults();
    if let (Some(base), Some(over)) = (out.as_object_mut(), partial.as_object()) {
        for (k, v) in over {
            base.insert(k.clone(), v.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_inherits_defaults() {
        let merged = spread_over_defaults(&serde_json::json!({"enabled": true}));
        assert_eq!(merged["enabled"], true);
        assert_eq!(merged["pixelValidation"], false);
        assert_eq!(merged["coordinateMode"], "pixels");
    }
}
