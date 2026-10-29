use crate::config::LspServerConfig;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

const MAX_MESSAGE_BYTES: usize = 2_000_000;

async fn read_packet<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Result<Value> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await? == 0 {
            bail!("LSP server closed pipe")
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                length = Some(value.trim().parse::<usize>()?);
            }
        }
    }
    let length = length.context("LSP message missing Content-Length")?;
    if length > MAX_MESSAGE_BYTES {
        bail!("LSP message exceeds size limit")
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    Ok(serde_json::from_slice(&payload)?)
}

async fn write_packet<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    value: &Value,
) -> Result<()> {
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_MESSAGE_BYTES {
        bail!("LSP request too large")
    }
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", data.len()).as_bytes())
        .await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}

struct LspClient {
    id: String,
    _child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    sequence: u64,
    versions: HashMap<PathBuf, u32>,
    language: String,
}

impl LspClient {
    async fn start(config: &LspServerConfig, root: &Path) -> Result<Self> {
        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        crate::subprocess_env::apply(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("Starting LSP {}", config.id))?;
        let input = child.stdin.take().context("LSP stdin missing")?;
        let output = BufReader::new(child.stdout.take().context("LSP stdout missing")?);
        let mut client = Self {
            id: config.id.clone(),
            _child: child,
            input,
            output,
            sequence: 1,
            versions: HashMap::new(),
            language: config.language_id.clone(),
        };
        let root_uri = reqwest::Url::from_file_path(root)
            .map_err(|_| anyhow::anyhow!("Invalid LSP workspace path"))?
            .to_string();
        client
            .request(
                "initialize",
                json!({
                    "processId":null,
                    "clientInfo":{"name":"atlas-agent","version":env!("CARGO_PKG_VERSION")},
                    "rootUri":root_uri,
                    "capabilities":{"textDocument":{"synchronization":{"didSave":true},
                        "definition":{},"references":{},"hover":{},
                        "documentSymbol":{},"implementation":{},"callHierarchy":{}}},
                    "workspaceFolders":[{"uri":root_uri,"name":"workspace"}]
                }),
            )
            .await?;
        client.notify("initialized", json!({})).await?;
        Ok(client)
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        write_packet(
            &mut self.input,
            &json!({"jsonrpc":"2.0","method":method,"params":params}),
        )
        .await
    }
    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.sequence;
        self.sequence += 1;
        write_packet(
            &mut self.input,
            &json!({
                "jsonrpc":"2.0","id":id,"method":method,"params":params
            }),
        )
        .await?;
        let result = timeout(Duration::from_secs(30), async {
            loop {
                let value = read_packet(&mut self.output).await?;
                if value.get("id").and_then(Value::as_u64) == Some(id) {
                    if let Some(error) = value.get("error") {
                        bail!("LSP {}: {error}", self.id)
                    }
                    return value.get("result").cloned().context("Missing LSP result");
                }
                if value.get("id").is_some() && value.get("method").is_some() {
                    write_packet(
                        &mut self.input,
                        &json!({
                            "jsonrpc":"2.0","id":value["id"],
                            "error":{"code":-32601,"message":"Client request unsupported"}
                        }),
                    )
                    .await?;
                }
            }
        })
        .await
        .context("LSP request timed out")?;
        result
    }

    async fn sync_document(&mut self, path: &Path, uri: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await?;
        if content.len() > 10_000_000 {
            bail!("LSP document too large")
        }
        if let Some(version) = self.versions.get(path).copied() {
            self.notify(
                "textDocument/didChange",
                json!({
                    "textDocument":{"uri":uri,"version":version+1},
                    "contentChanges":[{"text":content}]
                }),
            )
            .await?;
            self.versions.insert(path.to_path_buf(), version + 1);
        } else {
            self.notify(
                "textDocument/didOpen",
                json!({
                    "textDocument":{"uri":uri,"languageId":self.language,
                        "version":1,"text":content}
                }),
            )
            .await?;
            self.versions.insert(path.to_path_buf(), 1);
        }
        Ok(())
    }
}

pub struct LspManager {
    root: PathBuf,
    servers: Vec<(Vec<String>, LspClient)>,
}

impl LspManager {
    pub async fn connect(root: &Path, config: &[LspServerConfig]) -> Result<Self> {
        let mut servers = Vec::new();
        for profile in config {
            servers.push((
                profile.extensions.clone(),
                LspClient::start(profile, root).await?,
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            servers,
        })
    }
    pub fn definition() -> Value {
        json!({"type":"function","function":{
            "name":"lsp_query",
            "description":"Query a configured language server for definition, references, hover, symbols, implementation or call hierarchy.",
            "parameters":{"type":"object","properties":{
                "operation":{"type":"string","enum":[
                    "goToDefinition","findReferences","hover","documentSymbol",
                    "workspaceSymbol","goToImplementation","prepareCallHierarchy",
                    "incomingCalls","outgoingCalls"]},
                "filePath":{"type":"string"},"line":{"type":"integer","minimum":1},
                "character":{"type":"integer","minimum":1},"query":{"type":"string"}
            },"required":["operation","filePath"]}
        }})
    }
    pub async fn execute(&mut self, args: &Value) -> String {
        match self.query(args).await {
            Ok(value) => value.to_string().chars().take(16000).collect(),
            Err(error) => format!("ERROR: {error:#}"),
        }
    }
    async fn query(&mut self, args: &Value) -> Result<Value> {
        let raw = args
            .get("filePath")
            .and_then(Value::as_str)
            .context("LSP filePath required")?;
        let input = Path::new(raw);
        let path = if input.is_absolute() {
            input.to_path_buf()
        } else {
            self.root.join(input)
        };
        let resolved = path.canonicalize()?;
        if !resolved.starts_with(&self.root) {
            bail!("LSP file outside workspace")
        }
        let extension = resolved.extension().and_then(|s| s.to_str()).unwrap_or("");
        let (_, client) = self
            .servers
            .iter_mut()
            .find(|(extensions, _)| {
                extensions
                    .iter()
                    .any(|s| s.trim_start_matches('.') == extension)
            })
            .with_context(|| format!("No LSP server configured for .{extension}"))?;
        let uri = reqwest::Url::from_file_path(&resolved)
            .map_err(|_| anyhow::anyhow!("Invalid document path"))?
            .to_string();
        client.sync_document(&resolved, &uri).await?;
        let operation = args
            .get("operation")
            .and_then(Value::as_str)
            .context("LSP operation required")?;
        let line = args.get("line").and_then(Value::as_u64).unwrap_or(1);
        let character = args.get("character").and_then(Value::as_u64).unwrap_or(1);
        if line == 0 || character == 0 {
            bail!("LSP line and character are 1-based")
        }
        let text_document = json!({"uri":uri});
        let position = json!({"line":line-1,"character":character-1});
        let params = json!({"textDocument":text_document,"position":position});
        let result = match operation {
            "goToDefinition" => client.request("textDocument/definition", params).await?,
            "findReferences" => {
                client
                    .request(
                        "textDocument/references",
                        json!({
                            "textDocument":text_document,"position":position,
                            "context":{"includeDeclaration":true}
                        }),
                    )
                    .await?
            }
            "hover" => client.request("textDocument/hover", params).await?,
            "documentSymbol" => {
                client
                    .request(
                        "textDocument/documentSymbol",
                        json!({"textDocument":text_document}),
                    )
                    .await?
            }
            "workspaceSymbol" => {
                client
                    .request(
                        "workspace/symbol",
                        json!({
                            "query":args.get("query").and_then(Value::as_str).unwrap_or("")
                        }),
                    )
                    .await?
            }
            "goToImplementation" => {
                client
                    .request("textDocument/implementation", params)
                    .await?
            }
            "prepareCallHierarchy" => {
                client
                    .request("textDocument/prepareCallHierarchy", params)
                    .await?
            }
            "incomingCalls" | "outgoingCalls" => {
                let prepared = client
                    .request("textDocument/prepareCallHierarchy", params)
                    .await?;
                let item = prepared.as_array().and_then(|items| items.first()).cloned();
                if let Some(item) = item {
                    let method = if operation == "incomingCalls" {
                        "callHierarchy/incomingCalls"
                    } else {
                        "callHierarchy/outgoingCalls"
                    };
                    client.request(method, json!({"item":item})).await?
                } else {
                    json!([])
                }
            }
            other => bail!("Unsupported LSP operation: {other}"),
        };
        Ok(
            json!({"operation":operation,"filePath":resolved.display().to_string(),
            "resultCount":result.as_array().map(|v|v.len()),
            "result":result}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn content_length_framing_roundtrip() {
        let (mut writer, reader) = tokio::io::duplex(8192);
        let payload = json!({"jsonrpc":"2.0","id":5,"result":{"items":[1,2,3]}});
        write_packet(&mut writer, &payload).await.unwrap();
        let parsed = read_packet(&mut BufReader::new(reader)).await.unwrap();
        assert_eq!(parsed, payload);
    }
    #[tokio::test]
    async fn rejects_invalid_packet() {
        let (mut writer, reader) = tokio::io::duplex(8192);
        writer
            .write_all(b"Content-Length: 5\r\n\r\nhello")
            .await
            .unwrap();
        assert!(read_packet(&mut BufReader::new(reader)).await.is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn starts_language_server_synchronizes_document_and_returns_hover() {
        let root = tempfile::tempdir().unwrap();
        let doc = root.path().join("main.rs");
        tokio::fs::write(&doc, "fn main() {}\n").await.unwrap();
        let fake_server = r#"import sys,json
inp=sys.stdin.buffer
out=sys.stdout.buffer
while True:
    length=None
    while True:
        line=inp.readline()
        if not line:
            sys.exit(0)
        if line in (b'\r\n',b'\n'):
            break
        if line.lower().startswith(b'content-length:'):
            length=int(line.split(b':',1)[1].strip())
    if length is None:
        sys.exit(3)
    request=json.loads(inp.read(length))
    method=request.get('method')
    if 'id' not in request:
        continue
    if method=='initialize':
        result={'capabilities':{'hoverProvider':True}}
    elif method=='textDocument/hover':
        result={'contents':{'kind':'plaintext','value':'Hover from mock'}}
    else:
        result=None
    payload=json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}).encode()
    out.write(('Content-Length: %d\r\n\r\n'%len(payload)).encode()+payload)
    out.flush()
"#;
        let profiles = vec![LspServerConfig {
            id: "mock".into(),
            command: "python3".into(),
            args: vec!["-u".into(), "-c".into(), fake_server.into()],
            extensions: vec!["rs".into()],
            language_id: "rust".into(),
        }];
        let mut manager = LspManager::connect(root.path(), &profiles).await.unwrap();
        let result = manager
            .execute(&json!({"operation":"hover",
            "filePath":"main.rs","line":1,"character":1}))
            .await;
        assert!(result.contains("Hover from mock"), "{result}");
        let outside = tempfile::tempdir().unwrap();
        tokio::fs::write(outside.path().join("secret.rs"), "private")
            .await
            .unwrap();
        let attempt = manager
            .execute(&json!({"operation":"hover",
            "filePath":outside.path().join("secret.rs").display().to_string()}))
            .await;
        assert!(attempt.contains("ERROR"), "{attempt}");
    }
}
