//! Reuse attachment parsing locally without a remote Bridge, tokens or URL download.
//! SDK paths are confined to the project workspace even after symlink resolution.
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}};

const MAX_FILES: usize = 8;
const MAX_TEXT_BYTES: u64 = 1_048_576;
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 12 * 1024 * 1024;

fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else { None }
}

fn authorized_path(root: &Path, raw: &str) -> Result<PathBuf> {
    if raw.is_empty() || raw.contains("://") {
        bail!("Remote file attachments are disabled; provide a local workspace path");
    }
    let root = root.canonicalize().context("Cannot resolve workspace")?;
    let supplied = Path::new(raw);
    let file = if supplied.is_absolute() { supplied.to_path_buf() } else { root.join(supplied) };
    let file = file.canonicalize().with_context(|| format!("Cannot open local attachment: {raw}"))?;
    if !file.starts_with(&root) {
        bail!("Local attachment must remain inside the authorized workspace");
    }
    if !file.is_file() { bail!("Local attachment must be a regular file"); }
    Ok(file)
}

/// Fail before sending the user turn to a model if any attachment is invalid.
/// Local image inputs use existing multi-provider image blocks; text remains UTF-8.
pub fn attach(root: &Path, current: &Value, field: Option<&Value>) -> Result<Value> {
    let Some(field) = field else { return Ok(current.clone()); };
    if field.is_null() { return Ok(current.clone()); }
    let entries = field.as_array().context("file_attachments must be an array")?;
    if entries.is_empty() { return Ok(current.clone()); }
    if entries.len() > MAX_FILES { bail!("Too many local attachments (maximum {MAX_FILES})"); }
    let mut blocks = match current {
        Value::String(text) if text.is_empty() => Vec::new(),
        Value::String(text) => vec![json!({"type":"text","text":text})],
        Value::Array(items) => items.clone(),
        _ => bail!("SDK content must be a string or content blocks"),
    };
    let mut total = 0u64;
    for entry in entries {
        let raw = entry.as_str().or_else(|| {
            entry.get("path").or_else(|| entry.get("local_path")).and_then(Value::as_str)
        }).context("Remote file attachments are disabled; supply path or local_path")?;
        let path = authorized_path(root, raw)?;
        let name = path.file_name().and_then(|name| name.to_str()).context("Invalid attachment filename")?;
        let meta = fs::metadata(&path).context("Cannot stat local attachment")?;
        if meta.len() > MAX_IMAGE_BYTES { bail!("Attachment exceeds 5 MiB: {name}"); }
        total = total.checked_add(meta.len()).context("Attachment size overflow")?;
        if total > MAX_TOTAL_BYTES { bail!("Local attachments exceed 12 MiB"); }
        let bytes = fs::read(&path).with_context(|| format!("Cannot read local attachment: {name}"))?;
        if let Some(mime) = image_type(&bytes) {
            blocks.push(json!({"type":"text","text":format!("Attached local image: {name}")}));
            blocks.push(json!({"type":"image","source":{"type":"base64","media_type":mime,"data":STANDARD.encode(&bytes)}}));
        } else {
            if meta.len() > MAX_TEXT_BYTES { bail!("Local text attachment exceeds 1 MiB: {name}"); }
            let text = String::from_utf8(bytes).with_context(|| format!("Attachment {name} is neither a supported image nor UTF-8 text"))?;
            blocks.push(json!({"type":"text","text":format!("[Local attachment: {name}]\n{text}\n[/Local attachment]")}));
        }
    }
    Ok(Value::Array(blocks))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn images_and_text_follow_existing_multi_provider_parser() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("sample.txt"), "hello source").unwrap();
        fs::write(root.path().join("small.gif"), b"GIF89a00").unwrap();
        let data = attach(root.path(), &json!("Review"), Some(&json!([
            {"path":"sample.txt"}, {"local_path":"small.gif"}
        ]))).unwrap();
        let parsed = crate::multimodal::parse(&data).unwrap();
        assert!(parsed.preview.contains("hello source"));
        assert!(parsed.preview.contains("[image: image/gif]"));
        assert!(parsed.blocks.unwrap().iter().any(|block| block["type"] == "image"));
    }
    #[test]
    fn rejects_remote_and_workspace_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        for source in [
            json!([{"file_uuid":"remote-file","file_name":"private"}]),
            json!([{"path":"https://example.com/private"}]),
            json!([{"path":outside.path().join("secret.txt").to_string_lossy()}]),
        ] {
            assert!(attach(root.path(), &json!("Inspect"), Some(&source)).is_err());
        }
    }
    #[test]
    fn empty_field_preserves_previous_sdk_input() {
        assert_eq!(attach(Path::new("."), &json!("hello"), None).unwrap(), json!("hello"));
        assert_eq!(attach(Path::new("."), &json!("hello"), Some(&json!([]))).unwrap(), json!("hello"));
    }
}
