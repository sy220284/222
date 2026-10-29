//! MCP Prompts are user-invoked slash commands, not implicit system messages.
//! The original prompt command uses "/server:name (MCP) arg1 arg2" and binds
//! positional values against the prompt's named argument metadata.
use crate::slash_command;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{fs::{self, OpenOptions}, io::Write, path::Path};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub server: String,
    pub name: String,
    pub args: String,
}
pub fn parse(input: &str) -> Result<Option<Invocation>> {
    let Some(parsed) = slash_command::parse_slash_command(input) else {
        return Ok(None);
    };
    if !parsed.is_mcp {
        return Ok(None);
    }
    let raw = parsed
        .command_name
        .strip_suffix(" (MCP)")
        .context("Invalid MCP command marker")?;
    let (server, name) = raw
        .split_once(':')
        .context("MCP command must be /server:name (MCP)")?;
    if server.is_empty() || name.is_empty() {
        bail!("MCP command requires non-empty server and prompt names");
    }
    Ok(Some(Invocation {
        server: server.to_owned(),
        name: name.to_owned(),
        args: parsed.args,
    }))
}
pub fn named_arguments(prompt: &Value, args: &str) -> Result<Map<String, Value>> {
    let specs = match prompt.get("arguments") {
        None | Some(Value::Null) => return Ok(Map::new()),
        Some(Value::Array(specs)) => specs,
        _ => bail!("MCP prompt argument metadata must be an array"),
    };
    let pieces = args.split(' ').collect::<Vec<_>>();
    let mut result = Map::new();
    for (index, spec) in specs.iter().enumerate() {
        let name = spec
            .get("name")
            .and_then(Value::as_str)
            .context("MCP prompt argument missing name")?;
        if name.is_empty() {
            bail!("MCP prompt argument name is empty")
        }
        if result.contains_key(name) {
            bail!("Duplicate MCP prompt argument name")
        }
        if let Some(value) = pieces.get(index) {
            result.insert(name.to_owned(), Value::String((*value).to_owned()));
        }
    }
    Ok(result)
}
/// The original MCP client persists audio and non-image resource blobs and
/// returns a text block with the resulting file path. A random filename avoids
/// trusting a server-controlled URI or name as a filesystem path.
fn persist_mcp_blob(data: &str, mime: Option<&str>, workspace: &Path) -> Result<String> {
    let bytes = STANDARD.decode(data).context("MCP binary resource has invalid base64")?;
    let root = workspace.canonicalize().context("Cannot resolve MCP workspace")?;
    let directory = root.join(".atlas").join("tool-results");
    fs::create_dir_all(&directory).context("Cannot create MCP tool results directory")?;
    let directory = directory.canonicalize()?;
    if !directory.starts_with(&root) {
        bail!("MCP tool results directory escapes workspace");
    }
    let mime = mime.unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let extension = match mime.as_str() {
        "application/pdf" => "pdf",
        "application/json" => "json",
        "text/csv" => "csv",
        "text/plain" => "txt",
        "text/html" => "html",
        "text/markdown" => "md",
        "application/zip" => "zip",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/msword" => "doc",
        "application/vnd.ms-excel" => "xls",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        "audio/ogg" => "ogg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "image/svg+xml" => "svg",
        _ => "bin",
    };
    let path = directory.join(format!("mcp-blob-{}.{}", Uuid::new_v4(), extension));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(format!("Binary content ({}, {} bytes) saved to {}",
        if mime.is_empty() { "unknown type" } else { &mime },
        bytes.len(), path.display()))
}

fn binary_result_text(
    data: &str,
    mime: Option<&str>,
    prefix: &str,
    workspace: &Path,
) -> String {
    match persist_mcp_blob(data, mime, workspace) {
        Ok(message) => format!("{prefix}{message}"),
        Err(error) => format!(
            "{prefix}Binary content ({}, encoded length {} bytes) could not be saved to disk: {error:#}",
            mime.unwrap_or("unknown type"), data.len()
        ),
    }
}

pub fn render_response(server: &str, name: &str, result: &Value) -> Result<String> {
    let content = render_content(server, name, result)?;
    if content.blocks.is_some() {
        bail!("MCP prompt contains image blocks; use structured render_content");
    }
    Ok(content.preview)
}
pub fn render_content(
    server: &str,
    name: &str,
    result: &Value,
) -> Result<crate::multimodal::Input> {
    let workspace = std::env::current_dir().context("Cannot locate MCP prompt workspace")?;
    render_content_in_workspace(server, name, result, &workspace)
}

/// Render prompt blocks in the active Agent workspace, so saved MCP
/// attachments are reachable by the same Read and Shell tools as the prompt.
pub fn render_content_in_workspace(
    server: &str,
    name: &str,
    result: &Value,
    workspace: &Path,
) -> Result<crate::multimodal::Input> {
    let messages = result
        .get("messages")
        .and_then(Value::as_array)
        .context("MCP prompts/get response missing messages array")?;
    if messages.len() > 64 {
        bail!("MCP prompt returned too many messages")
    }
    let mut text = format!("[MCP prompt from {server}:{name}]\n");
    let mut blocks = vec![serde_json::json!({"type":"text","text":text})];
    let mut images = false;
    for msg in messages {
        let role = msg
            .get("role")
            .and_then(Value::as_str)
            .context("MCP prompt message missing role")?;
        if !matches!(role, "user" | "assistant") {
            bail!("Unsupported MCP prompt role")
        }
        let content = msg
            .get("content")
            .context("MCP prompt message missing content")?;
        let parts = if let Some(items) = content.as_array() {
            items.iter().collect::<Vec<_>>()
        } else {
            vec![content]
        };
        let prefix = format!("\n[{role}]\n");
        text.push_str(&prefix);
        blocks.push(serde_json::json!({"type":"text","text":prefix}));
        for (index, part) in parts.iter().enumerate() {
            match part.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let value = part
                        .get("text")
                        .and_then(Value::as_str)
                        .context("MCP prompt text block missing text")?;
                    let line = if index > 0 {
                        format!("\n{value}")
                    } else {
                        value.to_owned()
                    };
                    text.push_str(&line);
                    blocks.push(serde_json::json!({"type":"text","text":line}));
                }
                Some("image") => {
                    let mime = part
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .context("MCP image missing mimeType")?;
                    let data = part
                        .get("data")
                        .and_then(Value::as_str)
                        .context("MCP image missing data")?;
                    blocks.push(serde_json::json!({"type":"image","source":{
                        "type":"base64","media_type":mime,"data":data
                    }}));
                    images = true;
                    text.push_str(&format!("[image: {mime}]"));
                }
                Some("audio") => {
                    let data = part.get("data").and_then(Value::as_str)
                        .context("MCP audio block missing data")?;
                    let rendered = binary_result_text(
                        data, part.get("mimeType").and_then(Value::as_str),
                        &format!("[Audio from {server}] "), workspace,
                    );
                    text.push_str(&rendered);
                    blocks.push(serde_json::json!({"type":"text","text":rendered}));
                }
                Some("resource") => {
                    // Original client.ts::transformResultContent keeps text
                    // embedded in a resource, and forwards image blobs as
                    // images rather than dropping the prompt.
                    let resource = part
                        .get("resource")
                        .context("MCP prompt resource missing resource")?;
                    let uri = resource
                        .get("uri")
                        .and_then(Value::as_str)
                        .context("MCP prompt resource missing uri")?;
                    let prefix = format!("[Resource from {server} at {uri}] ");
                    if let Some(value) = resource.get("text").and_then(Value::as_str) {
                        let rendered = format!("{prefix}{value}");
                        text.push_str(&rendered);
                        blocks.push(serde_json::json!({"type":"text","text":rendered}));
                    } else if let Some(data) = resource.get("blob").and_then(Value::as_str) {
                        let mime = resource
                            .get("mimeType")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if !matches!(
                            mime,
                            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                        ) {
                            let rendered = binary_result_text(data, Some(mime), &prefix, workspace);
                            text.push_str(&rendered);
                            blocks.push(serde_json::json!({"type":"text","text":rendered}));
                            continue;
                        }
                        text.push_str(&prefix);
                        text.push_str(&format!("[image: {mime}]"));
                        blocks.push(serde_json::json!({"type":"text","text":prefix}));
                        blocks.push(serde_json::json!({"type":"image","source":{
                            "type":"base64","media_type":mime,"data":data
                        }}));
                        images = true;
                    } else {
                        bail!("MCP prompt resource has neither text nor blob");
                    }
                }
                Some("resource_link") => {
                    // Keep the original MCP link name, URI and description
                    // visible to the model as text; never fetch it implicitly.
                    let name = part
                        .get("name")
                        .and_then(Value::as_str)
                        .context("MCP resource link missing name")?;
                    let uri = part
                        .get("uri")
                        .and_then(Value::as_str)
                        .context("MCP resource link missing uri")?;
                    let mut rendered = format!("[Resource link: {name}] {uri}");
                    if let Some(description) = part.get("description").and_then(Value::as_str) {
                        if !description.is_empty() {
                            rendered.push_str(&format!(" ({description})"));
                        }
                    }
                    text.push_str(&rendered);
                    blocks.push(serde_json::json!({"type":"text","text":rendered}));
                }
                _ => bail!("Unsupported MCP prompt block type"),
            }
            if text.len() > 128_000 {
                bail!("MCP prompt exceeds 128 KB")
            }
        }
    }
    if images {
        // The same SDK validator is used for user and remote MCP images.
        // It rejects oversized, malformed and unsupported binary content.
        crate::multimodal::parse(&Value::Array(blocks))
    } else {
        Ok(crate::multimodal::Input {
            preview: text,
            blocks: None,
        })
    }
}

pub fn listed_commands(catalog: &Value) -> Result<Vec<Value>> {
    let services = catalog
        .get("servers")
        .and_then(Value::as_array)
        .context("MCP prompt catalog missing servers")?;
    let mut out = Vec::new();
    for server in services {
        if server["ok"] != true {
            continue;
        }
        let name = server
            .get("server")
            .and_then(Value::as_str)
            .context("MCP prompt catalog entry missing server")?;
        let prompts = server
            .get("items")
            .and_then(Value::as_array)
            .context("MCP prompt catalog entry missing items")?;
        for prompt in prompts {
            let id = prompt
                .get("name")
                .and_then(Value::as_str)
                .context("MCP prompt missing name")?;
            let args = match prompt.get("arguments") {
                Some(Value::Array(args)) => args
                    .iter()
                    .map(|arg| {
                        arg.get("name")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .context("MCP prompt argument missing name")
                    })
                    .collect::<Result<Vec<_>>>()?,
                None | Some(Value::Null) => Vec::new(),
                _ => bail!("MCP prompt argument metadata must be array"),
            };
            out.push(serde_json::json!({
                "name":format!("mcp__{}__{}",crate::runtime_contracts::normalize_mcp_name(name),id),
                "userFacingName":format!("{name}:{id} (MCP)"),
                "description":prompt.get("description").and_then(Value::as_str).unwrap_or(""),
                "argNames":args,
                "source":"mcp"
            }));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn mcp_image_blocks_are_adapted_to_same_multimodal_model_channel() {
        let result = render_content(
            "docs",
            "diagram",
            &json!({"messages":[
                {"role":"user","content":[
                    {"type":"text","text":"Review diagram"},
                    {"type":"image","mimeType":"image/png","data":"AQID"}
                ]}
            ]}),
        )
        .unwrap();
        let blocks = result.blocks.unwrap();
        let wire = crate::multimodal::openai(&blocks).unwrap();
        assert!(result.preview.contains("[image: image/png]"));
        assert_eq!(
            wire.last().unwrap()["image_url"]["url"],
            "data:image/png;base64,AQID"
        );
        assert!(render_content(
            "docs",
            "diagram",
            &json!({"messages":[{
                "role":"user","content":{"type":"image","mimeType":"image/png",
                    "data":"BAD!"}
            }]})
        )
        .is_err());
    }
    #[test]
    fn original_mcp_marker_and_literal_space_parameter_order() {
        let call = parse("/docs:inspect (MCP) first  third").unwrap().unwrap();
        assert_eq!(call.server, "docs");
        assert_eq!(call.name, "inspect");
        assert_eq!(call.args, "first  third");
        let names = json!({"arguments":[{"name":"a"},{"name":"b"},{"name":"c"},{"name":"d"}]});
        let args = named_arguments(&names, &call.args).unwrap();
        assert_eq!(args["a"], "first");
        assert_eq!(args["b"], "");
        assert_eq!(args["c"], "third");
        assert!(!args.contains_key("d"));
        assert_eq!(parse("ordinary prompt").unwrap(), None);
        assert_eq!(parse("/docs:inspect without marker").unwrap(), None);
        assert!(parse("/broken (MCP) hello").is_err());
    }
    #[test]
    fn preserves_role_order_and_rejects_non_text_instead_of_dropping() {
        let rendered = render_response(
            "docs",
            "inspect",
            &json!({"messages":[
                {"role":"user","content":{"type":"text","text":"hello"}},
                {"role":"assistant","content":{"type":"text","text":"next"}},
            ]}),
        )
        .unwrap();
        assert!(rendered.contains("[user]\nhello\n[assistant]\nnext"));
        assert!(render_response(
            "docs",
            "inspect",
            &json!({"messages":[
                {"role":"user","content":{"type":"image","data":"AQID"}}
            ]})
        )
        .is_err());
        assert!(named_arguments(&json!({"arguments":[{"name":"x"},{"name":"x"}]}), "a b").is_err());
    }
}
