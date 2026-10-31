//! Native Rust port derived from original: src/utils/dxt/helpers.ts
//!
//! DXT (.dxt/.mcpb) manifest parsing and validation. The original
//! delegates field validation to the upstream MCPB zod schema;
//! here the required-field/type checks of `McpbManifestSchema` are
//! reproduced directly (manifestVersion/name/displayName/author.name/
//! version), which covers the schema's required spine. Flattened error
//! formatting (`field: messages; formErrors`) mirrors the original.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

/// Validates a DXT manifest from a parsed JSON object, returning the
/// manifest unchanged on success or a flattened error string.
pub fn validate_manifest(manifest_json: &Value) -> Result<&Value, String> {
    let mut field_errors: Vec<String> = Vec::new();
    let mut form_errors: Vec<String> = Vec::new();

    if !manifest_json.is_object() {
        return Err("Invalid manifest: expected object".to_string());
    }

    let mut check_str = |key: &str| match manifest_json.get(key) {
        Some(v) if v.is_string() && !v.as_str().unwrap().is_empty() => {}
        Some(v) => field_errors.push(format!("{}: expected non-empty string, got {}", key, v)),
        None => field_errors.push(format!("{}: required", key)),
    };
    check_str("manifestVersion");
    check_str("name");
    match manifest_json.get("displayName") {
        Some(v) if v.is_string() && !v.as_str().unwrap().is_empty() => {}
        Some(v) => field_errors.push(format!("displayName: expected non-empty string, got {}", v)),
        None => field_errors.push("displayName: required".to_string()),
    }
    match manifest_json.get("version") {
        Some(v) if v.is_string() => {}
        Some(v) => field_errors.push(format!("version: expected string, got {}", v)),
        None => field_errors.push("version: required".to_string()),
    }
    match manifest_json.get("author") {
        Some(author) => match author.get("name") {
            Some(v) if v.is_string() && !v.as_str().unwrap().is_empty() => {}
            Some(v) => {
                field_errors.push(format!("author.name: expected non-empty string, got {}", v))
            }
            None => field_errors.push("author.name: required".to_string()),
        },
        None => field_errors.push("author: required".to_string()),
    }

    if field_errors.is_empty() && form_errors.is_empty() {
        Ok(manifest_json)
    } else {
        let messages: Vec<String> = field_errors
            .iter()
            .chain(form_errors.iter())
            .filter(|s| !s.is_empty())
            .cloned()
            .collect();
        Err(format!("Invalid manifest: {}", messages.join("; ")))
    }
}

/// Parses and validates a DXT manifest from raw text data.
pub fn parse_and_validate_manifest_from_text(manifest_text: &str) -> Result<Value, String> {
    let manifest_json: Value = serde_json::from_str(manifest_text)
        .map_err(|error| format!("Invalid JSON in manifest.json: {}", error))?;
    validate_manifest(&manifest_json).cloned()
}

/// Parses and validates a DXT manifest from raw binary data.
pub fn parse_and_validate_manifest_from_bytes(manifest_data: &[u8]) -> Result<Value, String> {
    let manifest_text = String::from_utf8_lossy(manifest_data).into_owned();
    parse_and_validate_manifest_from_text(&manifest_text)
}

fn sanitize_component(str_input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in str_input.to_lowercase().chars() {
        let c = match ch {
            ' ' | '\t' | '\n' | '\r' => '-',
            other => other,
        };
        let kept = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.';
        if !kept {
            continue;
        }
        if c == '-' {
            if last_dash {
                continue; // collapse runs of '-'
            }
            last_dash = true;
        } else {
            last_dash = false;
        }
        out.push(c);
    }
    // Trim leading/trailing dashes.
    out.trim_matches('-').to_string()
}

/// Generates an extension ID from author name and extension name, using
/// the same algorithm as the directory backend for consistency.
pub fn generate_extension_id(
    author_name: &str,
    extension_name: &str,
    prefix: Option<&str>,
) -> String {
    let sanitized_author = sanitize_component(author_name);
    let sanitized_name = sanitize_component(extension_name);
    match prefix {
        Some(prefix) => format!("{}.{}.{}", prefix, sanitized_author, sanitized_name),
        None => format!("{}.{}", sanitized_author, sanitized_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_manifest() -> Value {
        json!({
            "manifestVersion": "1.0",
            "name": "my-extension",
            "displayName": "My Extension",
            "version": "1.2.3",
            "author": { "name": "Alice" }
        })
    }

    #[test]
    fn valid_manifest_passes() {
        assert!(validate_manifest(&valid_manifest()).is_ok());
    }

    #[test]
    fn missing_fields_flatten() {
        let m = json!({ "name": "x" });
        let err = validate_manifest(&m).unwrap_err();
        assert!(err.contains("manifestVersion: required"), "{}", err);
        assert!(err.contains("author: required"), "{}", err);
        assert!(err.starts_with("Invalid manifest: "), "{}", err);
    }

    #[test]
    fn text_parse_rejects_bad_json() {
        let err = parse_and_validate_manifest_from_text("not json").unwrap_err();
        assert!(err.starts_with("Invalid JSON in manifest.json"));
    }

    #[test]
    fn bytes_parse_decodes_utf8() {
        let bytes = serde_json::to_vec(&valid_manifest()).unwrap();
        assert!(parse_and_validate_manifest_from_bytes(&bytes).is_ok());
    }

    #[test]
    fn extension_id_sanitizes() {
        assert_eq!(
            generate_extension_id("Alice Smith", "My Tool", Some("local.dxt")),
            "local.dxt.alice-smith.my-tool"
        );
        assert_eq!(generate_extension_id("A B", "C D", None), "a-b.c-d");
        // Special chars stripped, dash runs collapsed, edges trimmed.
        assert_eq!(sanitize_component("--Foo  Bar--!"), "foo-bar");
    }
}
