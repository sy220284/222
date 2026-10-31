//! Shared MCP tool/prompt content normalization and lossless output handling.
use crate::mcp_output_storage::{self as storage, ResultType};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Default)]
pub struct Output {
    pub text: String,
    pub blocks: Option<Vec<Value>>,
}
impl Output {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            blocks: None,
        }
    }
    pub fn hook_value(&self) -> Value {
        match &self.blocks {
            Some(blocks)=>json!(blocks.iter().map(|b|if b["type"]=="image" {json!({"type":"image","source":{"type":"base64","media_type":b["mime_type"],"data":b["data"]}})}else{b.clone()}).collect::<Vec<_>>()),
            None=>json!(self.text),
        }
    }
    pub fn prepend(&mut self, text: &str) {
        self.text.insert_str(0, text);
        if let Some(blocks) = self.blocks.as_mut() {
            blocks.insert(0, json!({"type":"text","text":text}));
        }
    }
    pub fn append(&mut self, text: &str) {
        self.text.push_str(text);
        if let Some(blocks) = self.blocks.as_mut() {
            blocks.push(json!({"type":"text","text":text}));
        }
    }
}
fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(_) => "[object Object]".into(),
        Value::Array(a) => a
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string(),
    }
}
pub fn infer_compact_schema(value: &Value, depth: i32) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(_) => "boolean".into(),
        Value::Number(_) => "number".into(),
        Value::String(_) => "string".into(),
        Value::Array(a) => a
            .first()
            .map(|v| format!("[{}]", infer_compact_schema(v, depth - 1)))
            .unwrap_or_else(|| "[]".into()),
        Value::Object(map) => {
            if depth <= 0 {
                return "{...}".into();
            }
            let mut entries = map.iter().collect::<Vec<_>>();
            // Object.entries puts canonical JS array-index keys first.
            entries.sort_by_key(|(key, _)| {
                key.parse::<u32>()
                    .ok()
                    .filter(|n| *n < u32::MAX && n.to_string() == **key)
                    .map(|n| (0, n))
                    .unwrap_or((1, 0))
            });
            let props = entries
                .iter()
                .take(10)
                .map(|(key, value)| format!("{key}: {}", infer_compact_schema(value, depth - 1)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{props}{}}}", if map.len() > 10 { ", ..." } else { "" })
        }
    }
}
fn binary(data: &str, mime: Option<&str>, prefix: &str, root: &Path) -> String {
    let bytes = match STANDARD.decode(data) {
        Ok(bytes) => bytes,
        Err(error) => {
            return format!(
                "{prefix}Binary content ({}) could not be decoded: {error}",
                mime.unwrap_or("unknown type")
            )
        }
    };
    match storage::persist_binary_content(
        root,
        &bytes,
        mime,
        &format!("mcp-blob-{}", uuid::Uuid::new_v4()),
    ) {
        Ok(saved) => storage::get_binary_blob_saved_message(
            &saved.filepath.to_string_lossy(),
            mime,
            saved.size,
            prefix,
        ),
        Err(error) => format!(
            "{prefix}Binary content ({}, {} bytes) could not be saved to disk: {error:#}",
            mime.unwrap_or("unknown type"),
            bytes.len()
        ),
    }
}
/// Return source-shaped blocks; the caller normalizes all images together so
/// whole-result image/byte limits apply before forwarding to a provider.
pub fn transform_block(block: &Value, server: &str, root: &Path) -> Result<Vec<Value>> {
    let text = |text: String| vec![json!({"type":"text","text":text})];
    match block["type"].as_str() {
        Some("text") => Ok(text(
            block["text"]
                .as_str()
                .context("MCP text block missing text")?
                .into(),
        )),
        Some("audio") => Ok(text(binary(
            block["data"].as_str().context("MCP audio missing data")?,
            block["mimeType"].as_str(),
            &format!("[Audio from {server}] "),
            root,
        ))),
        Some("image") => {
            let source = if let Some(source) = block.get("source") {
                source.clone()
            } else {
                json!({"type":"base64","media_type":block["mimeType"].as_str().or_else(||block["mime_type"].as_str()).context("MCP image missing mimeType")?,"data":block["data"].as_str().context("MCP image missing data")?})
            };
            Ok(vec![json!({"type":"image","source":source})])
        }
        Some("resource") => {
            let resource = block
                .get("resource")
                .context("MCP resource missing content")?;
            let prefix = format!(
                "[Resource from {server} at {}] ",
                resource["uri"]
                    .as_str()
                    .context("MCP resource missing URI")?
            );
            if let Some(value) = resource["text"].as_str() {
                return Ok(text(format!("{prefix}{value}")));
            }
            let data = resource["blob"]
                .as_str()
                .context("MCP resource has neither text nor blob")?;
            let mime = resource["mimeType"].as_str();
            if matches!(
                mime,
                Some("image/png" | "image/jpeg" | "image/gif" | "image/webp")
            ) {
                Ok(vec![
                    json!({"type":"text","text":prefix}),
                    json!({"type":"image","source":{"type":"base64","media_type":mime,"data":data}}),
                ])
            } else {
                Ok(text(binary(data, mime, &prefix, root)))
            }
        }
        Some("resource_link") => {
            let name = block["name"]
                .as_str()
                .context("MCP resource link missing name")?;
            let uri = block["uri"]
                .as_str()
                .context("MCP resource link missing URI")?;
            let description = block["description"]
                .as_str()
                .filter(|v| !v.is_empty())
                .map(|v| format!(" ({v})"))
                .unwrap_or_default();
            Ok(text(format!("[Resource link: {name}] {uri}{description}")))
        }
        _ => bail!("Unsupported MCP content block type"),
    }
}
pub fn normalized_blocks(blocks: &[Value], server: &str, root: &Path) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    for block in blocks {
        result.extend(transform_block(block, server, root)?);
    }
    if result.iter().any(|v| v["type"] == "image") {
        return crate::multimodal::parse(&json!(result))?
            .blocks
            .context("MCP image normalization lost blocks");
    }
    Ok(result)
}
pub fn from_blocks(blocks: Vec<Value>) -> Output {
    let text = blocks
        .iter()
        .map(|v| {
            if v["type"] == "image" {
                format!("[image: {}]", v["mime_type"].as_str().unwrap_or("unknown"))
            } else {
                v["text"].as_str().unwrap_or("").to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Output {
        text,
        blocks: Some(blocks),
    }
}
pub fn max_output_tokens(raw: Option<&str>) -> usize {
    raw.map(str::trim_start)
        .map(|v| v.strip_prefix('+').unwrap_or(v))
        .and_then(|v| {
            v.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<usize>()
                .ok()
        })
        .filter(|n| *n > 0)
        .unwrap_or(25_000)
}
fn truncate_text(text: &str, max: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += c.len_utf16();
            used <= max
        })
        .collect()
}
fn truncation_message(limit: usize) -> String {
    format!("\n\n[OUTPUT TRUNCATED - exceeded {limit} token limit]\n\nThe tool output was truncated. If this MCP server provides pagination or filtering tools, use them to retrieve specific portions of the data. If pagination is not available, inform the user that you are working with truncated output and results may be incomplete.")
}
pub fn process(result: &Value, tool: &str, server: &str, root: &Path) -> Result<Output> {
    process_with_limit(
        result,
        tool,
        server,
        root,
        max_output_tokens(std::env::var("MAX_MCP_OUTPUT_TOKENS").ok().as_deref()),
        !crate::env_runtime::is_env_defined_falsy(
            std::env::var("ENABLE_MCP_LARGE_OUTPUT_FILES")
                .ok()
                .as_deref(),
        ),
    )
}
pub fn process_with_limit(
    result: &Value,
    tool: &str,
    server: &str,
    root: &Path,
    token_limit: usize,
    save_large: bool,
) -> Result<Output> {
    // The source rejects isError before selecting structuredContent. Never
    // replace the diagnostic with a success-shaped JSON payload or save blobs.
    if result["isError"] == true {
        let details = if let Some(content) = result["content"].as_array().filter(|v| !v.is_empty())
        {
            content[0]["text"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_owned()
        } else {
            result
                .get("error")
                .map(js_string)
                .unwrap_or_else(|| "Unknown error".into())
        };
        return Ok(Output::text(format!("ERROR: {details}")));
    }

    let (plain, mut blocks, kind, schema) = if let Some(value) = result.get("toolResult") {
        (
            Some(js_string(value)),
            Vec::new(),
            ResultType::ToolResult,
            None,
        )
    } else if let Some(value) = result.get("structuredContent") {
        (
            Some(serde_json::to_string(value)?),
            Vec::new(),
            ResultType::StructuredContent,
            Some(infer_compact_schema(value, 2)),
        )
    } else if let Some(content) = result["content"].as_array() {
        let blocks = normalized_blocks(content, server, root)?;
        let schema = Some(infer_compact_schema(&json!(blocks), 2));
        (None, blocks, ResultType::ContentArray, schema)
    } else {
        bail!("MCP server \"{server}\" tool \"{tool}\": unexpected response format");
    };
    let size = plain
        .as_ref()
        .map(|v| v.encode_utf16().count())
        .unwrap_or_else(|| {
            blocks
                .iter()
                .map(|v| {
                    if v["type"] == "image" {
                        6400
                    } else {
                        v["text"].as_str().unwrap_or("").encode_utf16().count()
                    }
                })
                .sum()
        });
    let cap = token_limit.saturating_mul(4);
    let images = blocks.iter().any(|v| v["type"] == "image");
    let output = if server == "ide" || size <= cap {
        match plain {
            Some(v) => Output::text(v),
            None => from_blocks(blocks),
        }
    } else if save_large && !images {
        let data = match plain {
            Some(v) => v,
            None => serde_json::to_string_pretty(&blocks)?,
        };
        let length = data.encode_utf16().count();
        Output::text(match storage::persist_binary_content(root,data.as_bytes(),Some("text/plain"),&format!("mcp-output-{}",uuid::Uuid::new_v4())) {
            Ok(saved)=>storage::get_large_output_instructions(&saved.filepath.to_string_lossy(),length,&storage::get_format_description(kind,schema.as_deref()),None),
            Err(error)=>format!("Error: result ({length} characters) exceeds maximum allowed tokens. Failed to save output to file: {error:#}. If this MCP server provides pagination or filtering tools, use them to retrieve specific portions of the data."),
        })
    } else if let Some(text) = plain {
        Output::text(truncate_text(&text, cap) + &truncation_message(token_limit))
    } else {
        let mut remaining = cap;
        let mut clipped = Vec::new();
        for block in blocks.drain(..) {
            if block["type"] == "image" {
                if remaining >= 6400 {
                    clipped.push(block);
                    remaining -= 6400;
                } else {
                    clipped.push(json!({"type":"text","text":"[Image omitted: exceeds remaining MCP output budget]"}));
                }
            } else {
                let text = truncate_text(block["text"].as_str().unwrap_or(""), remaining);
                remaining = remaining.saturating_sub(text.encode_utf16().count());
                clipped.push(json!({"type":"text","text":text}));
                if remaining == 0 {
                    break;
                }
            }
        }
        clipped.push(json!({"type":"text","text":truncation_message(token_limit)}));
        from_blocks(clipped)
    };
    Ok(output)
}

pub fn from_hook_output(value: &Value, root: &Path) -> Result<Output> {
    match value {
        Value::String(text) => Ok(Output::text(text)),
        Value::Array(blocks) => Ok(from_blocks(normalized_blocks(
            blocks,
            "PostToolUse hook",
            root,
        )?)),
        Value::Object(_)
            if value.get("content").is_some()
                || value.get("toolResult").is_some()
                || value.get("structuredContent").is_some() =>
        {
            process(value, "updated output", "PostToolUse hook", root)
        }
        other => Ok(Output::text(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_schema_depth_width_and_js_string_precedence() {
        assert_eq!(
            infer_compact_schema(
                &json!({"title":"x","items":[{"id":3,"extra":{"nested":true}}]}),
                2
            ),
            "{title: string, items: [{...}]}"
        );
        assert_eq!(infer_compact_schema(&json!([]), 2), "[]");
        assert_eq!(
            infer_compact_schema(&json!({"z":true,"10":0,"2":1}), 2),
            "{2: number, 10: number, z: boolean}"
        );
        let mut wide = serde_json::Map::new();
        for n in 0..11 {
            wide.insert(format!("key{n}"), json!(n));
        }
        let schema = infer_compact_schema(&Value::Object(wide), 2);
        assert!(schema.ends_with(", ...}"));
        assert!(!schema.contains("key10:"));
        let root = tempfile::tempdir().unwrap();
        for (value, expected) in [
            (json!(null), "null"),
            (json!({"x":1}), "[object Object]"),
            (json!(["x", null, 3]), "x,,3"),
        ] {
            let result = process_with_limit(
                &json!({"toolResult":value,"structuredContent":{"wrong":true}}),
                "tool",
                "test",
                root.path(),
                25000,
                true,
            )
            .unwrap();
            assert_eq!(result.text, expected);
        }
    }
    #[test]
    fn large_structured_and_array_outputs_save_exact_content_and_schema() {
        for structured in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let text = "🦀".repeat(40);
            let value = if structured {
                json!({"items":[{"title":text}]})
            } else {
                json!([{"type":"text","text":text}])
            };
            let result = if structured {
                json!({"structuredContent":value})
            } else {
                json!({"content":value})
            };
            let output =
                process_with_limit(&result, "fetch", "test", root.path(), 10, true).unwrap();
            assert!(output.text.contains("100%"));
            assert!(output.text.contains("with schema:"));
            assert!(output.text.contains(if structured {
                "Format: JSON with"
            } else {
                "Format: JSON array with"
            }));
            let saved = std::fs::read_dir(root.path().join(".atlas/tool-results"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(
                serde_json::from_slice::<Value>(&std::fs::read(saved).unwrap()).unwrap(),
                value
            );
        }
    }
    #[test]
    fn text_truncation_is_explicit_and_never_splits_unicode_or_image_base64() {
        let root = tempfile::tempdir().unwrap();
        let output = process_with_limit(
            &json!({"toolResult":"🦀".repeat(30)}),
            "fetch",
            "test",
            root.path(),
            3,
            false,
        )
        .unwrap();
        assert!(output.text.starts_with(&"🦀".repeat(6)));
        assert!(output.text.contains("OUTPUT TRUNCATED"));
        assert!(!root.path().join(".atlas/tool-results").exists());
        let output=process_with_limit(&json!({"content":[{"type":"image","mimeType":"image/png","data":"AQID"},{"type":"text","text":"x".repeat(9000)}]}),"fetch","test",root.path(),2000,true).unwrap();
        let blocks = output.blocks.unwrap();
        assert_eq!(blocks[0]["data"], "AQID");
        assert_eq!(blocks[1]["text"].as_str().unwrap().len(), 1600);
        assert!(blocks.last().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("OUTPUT TRUNCATED"));
        assert!(!root.path().join(".atlas/tool-results").exists());
    }
    #[test]
    fn failed_binary_saves_and_invalid_payloads_are_visible_not_echoed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(".atlas"), "blocked").unwrap();
        let result = process_with_limit(
            &json!({"content":[{"type":"audio","data":"AAECAwQF","mimeType":"audio/mpeg"}]}),
            "fetch",
            "test",
            root.path(),
            25000,
            true,
        )
        .unwrap();
        assert!(result.text.contains("could not be saved"));
        assert!(!result.text.contains("AAECAwQF"));
        let result = process_with_limit(
            &json!({"content":[{"type":"audio","data":"BAD!"}]}),
            "fetch",
            "test",
            root.path(),
            25000,
            true,
        )
        .unwrap();
        assert!(result.text.contains("could not be decoded"));
        assert!(process_with_limit(
            &json!({"content":[{"type":"unknown"}]}),
            "fetch",
            "test",
            root.path(),
            25000,
            true
        )
        .is_err());
        for (input, expected) in [
            (Some(" +12suffix"), 12),
            (Some("-1"), 25000),
            (Some("0"), 25000),
            (None, 25000),
        ] {
            assert_eq!(max_output_tokens(input), expected);
        }
    }
    #[test]
    fn hook_context_keeps_image_order_and_standard_source_shape() {
        let root = tempfile::tempdir().unwrap();
        let blocks = normalized_blocks(
            &[
                json!({"type":"text","text":"before"}),
                json!({"type":"image","mimeType":"image/png","data":"AQID"}),
                json!({"type":"text","text":"after"}),
            ],
            "test",
            root.path(),
        )
        .unwrap();
        let mut output = from_blocks(blocks);
        output.prepend("warning");
        output.append("hook context");
        let wire = output.hook_value();
        assert_eq!(wire[2]["source"]["data"], "AQID");
        assert_eq!(wire[3]["text"], "after");
        assert_eq!(wire[4]["text"], "hook context");
        let replacement = from_hook_output(
            &json!({"content":[{"type":"text","text":"redacted"}]}),
            root.path(),
        )
        .unwrap();
        assert_eq!(replacement.text, "redacted");
        assert!(replacement
            .blocks
            .as_ref()
            .unwrap()
            .iter()
            .all(|b| b["type"] == "text"));
    }
}
