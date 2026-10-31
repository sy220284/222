//! Reuse attachment parsing locally without a remote Bridge, tokens or URL download.
//! SDK paths are confined to the project workspace even after symlink resolution.
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    } else {
        None
    }
}

fn authorized_path(root: &Path, raw: &str) -> Result<PathBuf> {
    if raw.is_empty() || raw.contains("://") {
        bail!("Remote file attachments are disabled; provide a local workspace path");
    }
    let root = root.canonicalize().context("Cannot resolve workspace")?;
    let supplied = Path::new(raw);
    let file = if supplied.is_absolute() {
        supplied.to_path_buf()
    } else {
        root.join(supplied)
    };
    let file = file
        .canonicalize()
        .with_context(|| format!("Cannot open local attachment: {raw}"))?;
    if !file.starts_with(&root) {
        bail!("Local attachment must remain inside the authorized workspace");
    }
    if !file.is_file() {
        bail!("Local attachment must be a regular file");
    }
    Ok(file)
}

/// Fail before sending the user turn to a model if any attachment is invalid.
/// Local image inputs use existing multi-provider image blocks; text remains UTF-8.
pub fn attach(
    root: &Path,
    current: &Value,
    field: Option<&Value>,
    authorize_read: impl Fn(&str, &Path) -> Result<()>,
) -> Result<Value> {
    let Some(field) = field else {
        return Ok(current.clone());
    };
    if field.is_null() {
        return Ok(current.clone());
    }
    let entries = field
        .as_array()
        .context("file_attachments must be an array")?;
    if entries.is_empty() {
        return Ok(current.clone());
    }
    if entries.len() > MAX_FILES {
        bail!("Too many local attachments (maximum {MAX_FILES})");
    }
    let mut blocks = match current {
        Value::String(text) if text.is_empty() => Vec::new(),
        Value::String(text) => vec![json!({"type":"text","text":text})],
        Value::Array(items) => items.clone(),
        _ => bail!("SDK content must be a string or content blocks"),
    };
    let mut attached = Vec::new();
    let mut total = 0u64;
    for entry in entries {
        let raw = entry
            .as_str()
            .or_else(|| {
                entry
                    .get("path")
                    .or_else(|| entry.get("local_path"))
                    .and_then(Value::as_str)
            })
            .context("Remote file attachments are disabled; supply path or local_path")?;
        let path = authorized_path(root, raw)?;
        // inboundAttachments.ts delegates @path to FileReadTool, whose
        // checkReadPermissionForTool applies deny/ask before reading bytes.
        // The local SDK uses that same permission engine before embedding.
        authorize_read(raw, &path)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("Invalid attachment filename")?;
        let meta = fs::metadata(&path).context("Cannot stat local attachment")?;
        if meta.len() > MAX_IMAGE_BYTES {
            bail!("Attachment exceeds 5 MiB: {name}");
        }
        total = total
            .checked_add(meta.len())
            .context("Attachment size overflow")?;
        if total > MAX_TOTAL_BYTES {
            bail!("Local attachments exceed 12 MiB");
        }
        let bytes =
            fs::read(&path).with_context(|| format!("Cannot read local attachment: {name}"))?;
        if let Some(mime) = image_type(&bytes) {
            attached.push(json!({"type":"text","text":format!("Attached local image: {name}")}));
            attached.push(json!({"type":"image","source":{"type":"base64","media_type":mime,"data":STANDARD.encode(&bytes)}}));
        } else {
            if meta.len() > MAX_TEXT_BYTES {
                bail!("Local text attachment exceeds 1 MiB: {name}");
            }
            let text = String::from_utf8(bytes).with_context(|| {
                format!("Attachment {name} is neither a supported image nor UTF-8 text")
            })?;
            attached.push(json!({"type":"text","text":format!("[Local attachment: {name}]\n{text}\n[/Local attachment]")}));
        }
    }
    // 2.1.88 inboundAttachments.ts prepends references to the LAST text
    // block, keeping it recognizable by slash-command expansion. Put the
    // local files before that same block; a trailing image must not erase
    // command detection or cause user text to be lost.
    let insertion = blocks
        .iter()
        .rposition(|item| item["type"] == "text")
        .unwrap_or(blocks.len());
    blocks.splice(insertion..insertion, attached);
    Ok(Value::Array(blocks))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn attach(root: &Path, current: &Value, field: Option<&Value>) -> Result<Value> {
        let executor = crate::tools::ToolExecutor::new(
            root.to_path_buf(),
            crate::tools::Approval::ReadOnly,
            false,
        );
        super::attach(root, current, field, |raw, path| {
            executor.require_read_rules(raw, path)
        })
    }
    #[test]
    fn images_and_text_follow_existing_multi_provider_parser() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("sample.txt"), "hello source").unwrap();
        fs::write(root.path().join("small.gif"), b"GIF89a00").unwrap();
        let data = attach(
            root.path(),
            &json!("Review"),
            Some(&json!([
                {"path":"sample.txt"}, {"local_path":"small.gif"}
            ])),
        )
        .unwrap();
        let parsed = crate::multimodal::parse(&data).unwrap();
        assert!(parsed.preview.contains("hello source"));
        assert!(parsed.preview.contains("[image: image/gif]"));
        assert!(parsed
            .blocks
            .unwrap()
            .iter()
            .any(|block| block["type"] == "image"));
    }
    #[test]
    fn local_attachments_preserve_last_user_text_and_slash_command() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("reference.txt"), "reference body").unwrap();
        let original = json!([
            {"type":"text","text":"earlier context"},
            {"type":"text","text":"/summarize this source"},
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"iVBORw0KGgo="}}
        ]);
        let result = attach(
            root.path(),
            &original,
            Some(&json!([{"path":"reference.txt"}])),
        )
        .unwrap();
        let items = result.as_array().unwrap();
        let text_blocks = items
            .iter()
            .filter(|b| b["type"] == "text")
            .collect::<Vec<_>>();
        assert_eq!(
            text_blocks.last().unwrap()["text"],
            "/summarize this source"
        );
        assert!(items.iter().any(|b| b["text"]
            .as_str()
            .is_some_and(|s| s.contains("reference body"))));
        assert_eq!(items.last().unwrap()["type"], "image");
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
        assert_eq!(
            attach(Path::new("."), &json!("hello"), None).unwrap(),
            json!("hello")
        );
        assert_eq!(
            attach(Path::new("."), &json!("hello"), Some(&json!([]))).unwrap(),
            json!("hello")
        );
    }
}
