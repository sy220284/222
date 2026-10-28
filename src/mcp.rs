use crate::{config::McpServerConfig, protocol::ToolCall};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::HashMap, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);

struct Client {
    name: String,
    _child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    async fn connect(server: &McpServerConfig) -> Result<Self> {
        let mut process = Command::new(&server.command);
        process.args(&server.args).stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true);
        let mut child = process.spawn()
            .with_context(|| format!("Starting MCP server {}", server.id))?;
        let input = child.stdin.take().context("MCP stdin unavailable")?;
        let output = BufReader::new(child.stdout.take().context("MCP stdout unavailable")?);
        let mut client = Self { name: server.id.clone(), _child: child, input, output, id: 1 };
        timeout(RPC_TIMEOUT, client.request("initialize", json!({
            "protocolVersion":"2025-03-26",
            "capabilities":{},
            "clientInfo":{"name":"atlas-agent","version":env!("CARGO_PKG_VERSION")}
        }))).await.context("MCP initialize timed out")??;
        client.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await?;
        Ok(client)
    }

    async fn send(&mut self, message: &Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(message)?;
        bytes.push(b'\n');
        self.input.write_all(&bytes).await?;
        self.input.flush().await?;
        Ok(())
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.id;
        self.id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})).await?;
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line).await? == 0 {
                bail!("MCP server {} closed stdout during {method}", self.name);
            }
            let message: Value = serde_json::from_str(&line)
                .with_context(|| format!("MCP server {} returned non-JSON data", self.name))?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    bail!("MCP server {} error: {}", self.name, error);
                }
                return message.get("result").cloned().context("MCP response missing result");
            }
            if message.get("id").is_some() && message.get("method").is_some() {
                self.send(&json!({"jsonrpc":"2.0","id":message["id"],
                    "error":{"code":-32601,"message":"Unsupported client method"}})).await?;
            }
        }
    }
}

pub struct McpManager {
    clients: Vec<Client>,
    tools: Vec<Value>,
    routes: HashMap<String, (usize, String)>,
}

impl McpManager {
    pub async fn connect(servers: &[McpServerConfig]) -> Result<Self> {
        if servers.len() > 16 { bail!("Too many MCP servers") }
        let mut out = Self { clients: Vec::new(), tools: Vec::new(), routes: HashMap::new() };
        for server in servers {
            let mut client = Client::connect(server).await?;
            let index = out.clients.len();
            let mut cursor: Option<String> = None;
            for _ in 0..20 {
                let params = match &cursor {
                    Some(value) => json!({"cursor":value}),
                    None => json!({}),
                };
                let result = timeout(RPC_TIMEOUT, client.request("tools/list", params))
                    .await.context("MCP discovery timed out")??;
                let tools = result.get("tools").and_then(Value::as_array)
                    .context("MCP tools/list missing tools array")?;
                for tool in tools {
                    let name = tool.get("name").and_then(Value::as_str)
                        .context("MCP tool missing name")?;
                    let alias = format!("mcp_{}_{}", server.id.replace('-', "_"), out.routes.len());
                    let description = format!("MCP {}: {}. Requires approval.",
                        server.id, tool.get("description").and_then(Value::as_str).unwrap_or(name));
                    let parameters = tool.get("inputSchema").cloned()
                        .unwrap_or(json!({"type":"object","properties":{}}));
                    out.tools.push(json!({"type":"function","function":{
                        "name":alias,"description":description,"parameters":parameters
                    }}));
                    out.routes.insert(alias, (index, name.to_owned()));
                }
                cursor = result.get("nextCursor").and_then(Value::as_str).map(str::to_owned);
                if cursor.is_none() { break; }
            }
            if cursor.is_some() { bail!("MCP server {} returned too many pages", server.id) }
            out.clients.push(client);
        }
        Ok(out)
    }

    pub fn definitions(&self) -> Vec<Value> { self.tools.clone() }
    pub fn handles(&self, name: &str) -> bool { self.routes.contains_key(name) }

    pub async fn execute(&mut self, call: &ToolCall) -> String {
        match self.call(call).await {
            Ok(content) => content,
            Err(error) => format!("ERROR: {error:#}"),
        }
    }

    async fn call(&mut self, call: &ToolCall) -> Result<String> {
        let (index, real_name) = self.routes.get(&call.function.name)
            .context("Unknown MCP tool")?.clone();
        let args: Value = serde_json::from_str(&call.function.arguments)?;
        let response = timeout(RPC_TIMEOUT, self.clients[index].request("tools/call",
            json!({"name":real_name,"arguments":args})))
            .await.context("MCP tool call timed out")??;
        let output: Vec<&str> = response.get("content").and_then(Value::as_array)
            .map(|parts| parts.iter().filter_map(|part| part.get("text").and_then(Value::as_str)).collect())
            .unwrap_or_default();
        let result = if output.is_empty() { response.to_string() } else { output.join("\n") };
        Ok(result.chars().take(16_000).collect())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::protocol::FunctionCall;

    #[tokio::test]
    async fn discovers_and_calls_stdio_tool() {
        let script = r#"while IFS= read -r line; do
case "$line" in
  *'"method":"initialize"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"mock","version":"1"}}}'
    ;;
  *'"method":"tools/list"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"Echo string","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}]}}'
    ;;
  *'"method":"tools/call"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"echo ok"}]}}'
    ;;
esac
done"#;
        let servers = vec![McpServerConfig {
            id: "mock".to_owned(), command: "sh".to_owned(),
            args: vec!["-c".to_owned(), script.to_owned()],
        }];
        let mut manager = McpManager::connect(&servers).await.unwrap();
        assert_eq!(manager.definitions().len(), 1);
        assert!(manager.handles("mcp_mock_0"));
        let call = ToolCall {
            id: "call1".to_owned(), kind: "function".to_owned(),
            function: FunctionCall {
                name: "mcp_mock_0".to_owned(),
                arguments: json!({"text":"hello"}).to_string(),
            },
        };
        assert_eq!(manager.execute(&call).await, "echo ok");
    }
}
