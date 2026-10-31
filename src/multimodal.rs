//! Bounded, provider-neutral SDK text/image input.
//! Only locally supplied base64 images are accepted; never fetch remote URLs
//! while parsing a message. Keep originals in session storage, not in logs.
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
const MAX_IMAGE_DATA: usize = 5 * 1024 * 1024;
const MAX_TOTAL_DATA: usize = 12 * 1024 * 1024;
const MAX_IMAGES: usize = 8;

pub struct Input {
    pub preview: String,
    pub blocks: Option<Vec<Value>>,
}
fn media_type(value: &str) -> Result<&str> {
    match value {
        "image/png" | "image/jpeg" | "image/webp" | "image/gif" => Ok(value),
        _ => bail!("Unsupported image media type"),
    }
}
fn validate_base64(data: &str) -> Result<()> {
    if data.is_empty() || data.len() > MAX_IMAGE_DATA || data.len() % 4 != 0 {
        bail!("Image base64 data has an invalid length");
    }
    let mut padding = false;
    let mut pad = 0;
    for c in data.bytes() {
        if c == b'=' {
            padding = true;
            pad += 1;
            if pad > 2 {
                bail!("Invalid image padding")
            }
        } else if padding || !(c.is_ascii_alphanumeric() || c == b'+' || c == b'/') {
            bail!("Invalid image base64 encoding");
        }
    }
    Ok(())
}
/// Equivalent of the inbound bridge image source normalization in 2.1.88.
/// Preserve an explicit snake_case MIME, then accept mobile camelCase, and
/// finally sniff magic bytes when the client omitted both. Validation and
/// size limits are enforced by the caller before any decoding occurs.
fn inbound_image_media_type(source: &Value, data: &str) -> Result<String> {
    if let Some(explicit) = source
        .get("media_type")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Ok(media_type(explicit)?.to_owned());
    }
    if let Some(camel) = source
        .get("mediaType")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Ok(media_type(camel)?.to_owned());
    }
    let prefix = data.get(..data.len().min(24)).unwrap_or("");
    // Prefix decoding may fail when truncated at the 24-character boundary;
    // all length-limited inputs are already validated by validate_base64.
    let bytes = STANDARD.decode(prefix).unwrap_or_default();
    let detected = if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF") {
        "image/gif"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        // The 2.1.88 imageResizer source also defaults unknown signatures to PNG.
        "image/png"
    };
    Ok(detected.to_owned())
}

/// Source inboundMessages.ts contract: ignore non-user messages and empty
/// content, preserve a string UUID and leave payloads for parse() validation.
pub fn extract_inbound_user_fields(message: &Value) -> Option<(Value, Option<String>)> {
    if message.get("type")?.as_str()? != "user" {
        return None;
    }
    let content = message.get("message")?.get("content")?;
    if content.is_null()
        || content.as_str().is_some_and(str::is_empty)
        || content.as_array().is_some_and(Vec::is_empty)
    {
        return None;
    }
    Some((
        content.clone(),
        message
            .get("uuid")
            .and_then(Value::as_str)
            .map(str::to_owned),
    ))
}

/// Source: 2.1.88 utils/imageResizer.ts overDim precheck (401-410). A PNG
/// stores its IHDR width/height at fixed offsets; read them without a full
/// decode so oversized dimensions get a pixel-dimension error instead of a
/// generic byte-size message. Size-under-5MB does not imply
/// dimensions-under-cap, and dimension checks are useful even above 5MB.
const IMAGE_MAX_WIDTH: u32 = 2000;
const IMAGE_MAX_HEIGHT: u32 = 2000;

fn png_over_dimensions(base64_data: &str) -> Option<(u32, u32)> {
    let raw = STANDARD.decode(base64_data).ok()?;
    // PNG signature is 8 bytes, IHDR width/height live at offsets 16-24.
    if raw.len() < 24 || raw[0] != 0x89 || raw[1] != 0x50 || raw[2] != 0x4e || raw[3] != 0x47 {
        return None;
    }
    let width = u32::from_be_bytes([raw[16], raw[17], raw[18], raw[19]]);
    let height = u32::from_be_bytes([raw[20], raw[21], raw[22], raw[23]]);
    (width > IMAGE_MAX_WIDTH || height > IMAGE_MAX_HEIGHT).then_some((width, height))
}

/// Source: 2.1.88 utils/imageValidation.ts::validateImagesForAPI.
/// Validate the entire outgoing user history at the model boundary. The limit
/// applies to encoded base64 *characters*, not decoded bytes. The importer
/// normalizes image blocks, while resumed sessions may retain raw SDK blocks.
pub fn validate_images_for_api(messages: &[crate::protocol::Message]) -> Result<()> {
    let mut oversized = Vec::new();
    let mut image_index = 0usize;
    for message in messages {
        if message.role != "user" {
            continue;
        }
        let Some(blocks) = message.content_blocks.as_ref() else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("image") {
                continue;
            }
            let data = block
                .get("source")
                .filter(|source| source.get("type").and_then(Value::as_str) == Some("base64"))
                .and_then(|source| source.get("data"))
                .or_else(|| block.get("data"))
                .and_then(Value::as_str);
            let Some(data) = data else {
                continue;
            };
            image_index += 1;
            if data.len() > MAX_IMAGE_DATA {
                oversized.push((image_index, data.len()));
            }
        }
    }
    let max_size = crate::display_format::format_file_size(MAX_IMAGE_DATA as f64);
    // The source resizer attempts compression and only reports dimension
    // overflow after it fails. Without a local decode engine the pixel check
    // runs first for PNGs, so oversized dimensions get the precise guidance.
    let dimension_oversized = oversized
        .iter()
        .filter_map(|(index, size)| {
            png_over_dimensions(oversized_data(messages, *index))
                .map(|(width, height)| (*index, *size, width, height))
        })
        .collect::<Vec<_>>();
    if let [(_, size, width, height)] = dimension_oversized.as_slice() {
        bail!(
            "Image base64 size ({}) exceeds API limit ({max_size}); PNG header reports {width}x{height}px, \
             above the {IMAGE_MAX_WIDTH}x{IMAGE_MAX_HEIGHT}px cap. \
             Please resize the image to reduce its pixel dimensions before sending.",
            crate::display_format::format_file_size(*size as f64),
        );
    }
    match oversized.as_slice() {
        [] => Ok(()),
        [(_, size)] => bail!(
            "Image base64 size ({}) exceeds API limit ({max_size}). Please resize the image before sending.",
            crate::display_format::format_file_size(*size as f64),
        ),
        _ => {
            let details = oversized
                .iter()
                .map(|(index, size)| {
                    format!(
                        "Image {index}: {}",
                        crate::display_format::format_file_size(*size as f64)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "{} images exceed the API limit ({max_size}): {details}. Please resize these images before sending.",
                oversized.len(),
            )
        }
    }
}

/// Fetch the base64 payload of the nth (1-based) oversized image block so the
/// dimension precheck can inspect its PNG header.
fn oversized_data(messages: &[crate::protocol::Message], target: usize) -> &str {
    let mut image_index = 0usize;
    for message in messages {
        if message.role != "user" {
            continue;
        }
        let Some(blocks) = message.content_blocks.as_ref() else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("image") {
                continue;
            }
            if let Some(data) = block
                .get("source")
                .filter(|source| source.get("type").and_then(Value::as_str) == Some("base64"))
                .and_then(|source| source.get("data"))
                .or_else(|| block.get("data"))
                .and_then(Value::as_str)
            {
                image_index += 1;
                if image_index == target {
                    return data;
                }
            }
        }
    }
    ""
}

pub fn parse(value: &Value) -> Result<Input> {
    if let Some(text) = value.as_str() {
        if text.len() > MAX_TOTAL_DATA {
            bail!("SDK text input too large")
        }
        return Ok(Input {
            preview: text.to_owned(),
            blocks: None,
        });
    }
    let entries = value
        .as_array()
        .context("SDK content must be a string or content blocks")?;
    if entries.is_empty() {
        bail!("SDK content blocks cannot be empty")
    }
    let mut normalized = Vec::new();
    let mut preview = Vec::new();
    let mut images = 0usize;
    let mut size = 0usize;
    for (index, entry) in entries.iter().enumerate() {
        match entry.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = entry
                    .get("text")
                    .and_then(Value::as_str)
                    .with_context(|| format!("Content block {index} missing text"))?;
                size = size.saturating_add(text.len());
                normalized.push(json!({"type":"text","text":text}));
                preview.push(text.to_owned());
            }
            Some("image") => {
                let source = entry.get("source").context("Image requires source")?;
                if source.get("type").and_then(Value::as_str) != Some("base64") {
                    bail!("SDK image requires a base64 source");
                }
                let data = source
                    .get("data")
                    .and_then(Value::as_str)
                    .context("Image missing data")?;
                validate_base64(data)?;
                let mime = inbound_image_media_type(source, data)?;
                images += 1;
                if images > MAX_IMAGES {
                    bail!("SDK input contains too many images")
                }
                size = size.saturating_add(data.len());
                normalized.push(json!({"type":"image","mime_type":mime,"data":data}));
                preview.push(format!("[image: {mime}]"));
            }
            _ => bail!("Unsupported SDK content block {index}"),
        }
        if size > MAX_TOTAL_DATA {
            bail!("SDK content exceeds 12 MB")
        }
    }
    Ok(Input {
        preview: preview.join("\n"),
        blocks: Some(normalized),
    })
}
pub fn openai(blocks: &[Value]) -> Result<Vec<Value>> {
    let mut result = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => result.push(json!({"type":"text","text":block["text"]})),
            Some("image") => {
                let mime = media_type(
                    block["mime_type"]
                        .as_str()
                        .context("Image has no media type")?,
                )?;
                let data = block["data"].as_str().context("Image has no data")?;
                validate_base64(data)?;
                result.push(json!({"type":"image_url",
                    "image_url":{"url":format!("data:{mime};base64,{data}")}}));
            }
            _ => bail!("Unsupported content block for OpenAI"),
        }
    }
    Ok(result)
}
pub fn gemini(blocks: &[Value]) -> Result<Vec<Value>> {
    let mut result = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => result.push(json!({"text":block["text"]})),
            Some("image") => {
                let mime = media_type(
                    block["mime_type"]
                        .as_str()
                        .context("Image has no media type")?,
                )?;
                let data = block["data"].as_str().context("Image has no data")?;
                validate_base64(data)?;
                result.push(json!({"inlineData":{"mimeType":mime,"data":data}}));
            }
            _ => bail!("Unsupported content block for Gemini"),
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_header_precheck_reads_ihdr_dimensions() {
        // Minimal PNG: 8-byte signature + IHDR chunk with 3000x1000.
        let mut raw = vec![0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        raw.extend_from_slice(&[0, 0, 0, 13]); // IHDR length
        raw.extend_from_slice(b"IHDR");
        raw.extend_from_slice(&3000u32.to_be_bytes());
        raw.extend_from_slice(&1000u32.to_be_bytes());
        raw.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth, color, etc.
        let encoded = STANDARD.encode(&raw);
        assert_eq!(
            png_over_dimensions(&encoded),
            Some((3000, 1000)),
            "width above the 2000px cap must be detected"
        );
        // Under the cap -> None.
        raw[16..20].copy_from_slice(&2000u32.to_be_bytes());
        raw[20..24].copy_from_slice(&1000u32.to_be_bytes());
        assert_eq!(
            png_over_dimensions(&STANDARD.encode(&raw)),
            None,
            "dimensions within the cap pass"
        );
        // Non-PNG data never reports dimensions.
        assert_eq!(png_over_dimensions(&STANDARD.encode(b"plain text")), None);
    }

    #[test]
    fn preserves_text_and_images_without_exposing_binary_in_preview() {
        let input = parse(&json!([
            {"type":"text","text":"What is visible?"},
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AQID"}}
        ]))
        .unwrap();
        assert!(input.preview.contains("[image: image/png]"));
        assert!(!input.preview.contains("AQID"));
        let blocks = input.blocks.unwrap();
        assert_eq!(
            openai(&blocks).unwrap()[1]["image_url"]["url"],
            "data:image/png;base64,AQID"
        );
        assert_eq!(gemini(&blocks).unwrap()[1]["inlineData"]["data"], "AQID");
    }
    #[test]
    fn bridge_inbound_normalizes_ios_media_type_and_sniffs_missing_type() {
        let cases = [
            ("iVBORw0KGgo=", "image/png"),
            ("/9j/AA==", "image/jpeg"),
            ("R0lGODlh", "image/gif"),
            ("UklGRgAAAABXRUJQ", "image/webp"),
            ("AQID", "image/png"),
        ];
        for (bytes, want) in cases {
            let input =
                parse(&json!([{"type":"image","source":{"type":"base64","data":bytes}}])).unwrap();
            assert_eq!(input.blocks.as_ref().unwrap()[0]["mime_type"], want);
        }
        let explicit = parse(&json!([{"type":"image","source":{"type":"base64",
            "data":"AQID","mediaType":"image/gif"}}]))
        .unwrap();
        assert_eq!(explicit.blocks.unwrap()[0]["mime_type"], "image/gif");
        let preferred = parse(&json!([{"type":"image","source":{"type":"base64",
            "data":"AQID","mediaType":"image/gif","media_type":"image/jpeg"}}]))
        .unwrap();
        assert_eq!(preferred.blocks.unwrap()[0]["mime_type"], "image/jpeg");
        assert!(parse(&json!([{"type":"image","source":{"type":"base64",
            "data":"AQID","mediaType":"application/pdf"}}]))
        .is_err());
    }

    #[test]
    fn api_image_validation_scans_all_user_messages_and_reports_positions() {
        use crate::protocol::Message;
        let mut small = Message::text("user", "small");
        small.content_blocks = Some(vec![json!({
            "type":"image", "data":"AQID", "mime_type":"image/png"
        })]);
        let mut first = Message::text("user", "large");
        first.content_blocks = Some(vec![json!({
            "type":"image",
            "source":{"type":"base64","data":"A".repeat(MAX_IMAGE_DATA + 4)}
        })]);
        let mut second = Message::text("user", "large again");
        second.content_blocks = Some(vec![json!({
            "type":"image","data":"A".repeat(MAX_IMAGE_DATA + 8)
        })]);
        let mut assistant = Message::text("assistant", "ignored");
        assistant.content_blocks = second.content_blocks.clone();
        assert!(validate_images_for_api(&[small.clone(), assistant]).is_ok());
        let error = validate_images_for_api(&[small, first, second])
            .unwrap_err()
            .to_string();
        assert!(error.contains("2 images exceed the API limit"), "{error}");
        assert!(error.contains("Image 2:"), "{error}");
        assert!(error.contains("Image 3:"), "{error}");
    }

    #[test]
    fn inbound_user_field_extraction_preserves_uuid_and_skips_unsupported() {
        for invalid in [
            json!({"type":"assistant","message":{"content":"hi"}}),
            json!({"type":"user","message":{"content":""}}),
            json!({"type":"user","message":{"content":[]}}),
            json!({"type":"user","message":{}}),
        ] {
            assert!(extract_inbound_user_fields(&invalid).is_none());
        }
        let (content, uuid) = extract_inbound_user_fields(&json!({"type":"user","uuid":"id-1",
            "message":{"content":[{"type":"text","text":"hi"}]}}))
        .unwrap();
        assert_eq!(content, json!([{"type":"text","text":"hi"}]));
        assert_eq!(uuid.as_deref(), Some("id-1"));
    }

    #[test]
    fn rejects_bad_payload_before_network_or_queue_mutation() {
        for input in [
            json!([{"type":"image","source":{"type":"url","url":"https://example.org/image.png"}}]),
            json!([{"type":"image","source":{"type":"base64","media_type":"application/pdf","data":"AQID"}}]),
            json!([{"type":"image","source":{"type":"base64","media_type":"image/png","data":"INVALID!"}}]),
            json!([{"type":"tool_result","content":"untrusted"}]),
            json!([]),
        ] {
            assert!(parse(&input).is_err(), "{input}");
        }
    }
}
