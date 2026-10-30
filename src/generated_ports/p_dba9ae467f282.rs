//! Rust exports derived from original: src/services/remoteManagedSettings/types.ts
//!
//! Full equivalence at the type/validation layer: the response shape is
//! uuid + checksum + a permissive settings record (full SettingsSchema
//! validation happens at the fetch layer in the original index.ts), plus the
//! fetch result with cache-valid 304 semantics.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `RemoteManagedSettingsResponse` — permissive settings record to avoid a
/// circular dependency with the full settings schema (original note kept).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteManagedSettingsResponse {
    /// Settings UUID
    pub uuid: String,
    pub checksum: String,
    pub settings: Value,
}

impl RemoteManagedSettingsResponse {
    /// zod object equivalence: all three keys present with correct types.
    pub fn is_valid(value: &Value) -> bool {
        let Some(obj) = value.as_object() else {
            return false;
        };
        obj.get("uuid").is_some_and(Value::is_string)
            && obj.get("checksum").is_some_and(Value::is_string)
            && obj.get("settings").is_some_and(Value::is_object)
    }
}

/// `RemoteManagedSettingsFetchResult` — `settings: None` means 304 Not
/// Modified (cache is valid).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteManagedSettingsFetchResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// If true, don't retry on failure (e.g., auth errors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_retry: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_shape_validation() {
        let ok = serde_json::json!({
            "uuid": "u1", "checksum": "c1", "settings": {}
        });
        assert!(RemoteManagedSettingsResponse::is_valid(&ok));
        assert!(!RemoteManagedSettingsResponse::is_valid(
            &serde_json::json!({
                "uuid": "u1"
            })
        ));
    }
}
