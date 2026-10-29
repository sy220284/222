//! MCP Prompts are user-invoked slash commands, not implicit system messages.
//! The original prompt command uses "/server:name (MCP) arg1 arg2" and binds
//! positional values against the prompt's named argument metadata.
use crate::slash_command;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

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
