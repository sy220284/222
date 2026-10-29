use crate::{config::McpServerConfig, protocol::ToolCall, compat_helpers, runtime_contracts};
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
    server_info: Option<Value>,
    tools_changed: bool,
}

impl Client {
    async fn connect(server: &McpServerConfig) -> Result<Self> {
        let expand = |raw: &str| -> Result<String> {
            let data=compat_helpers::expand_env_vars_in_string(raw);
            if !data.missing_vars.is_empty() {
                bail!("MCP {} missing environment variables: {}",
                    server.id,data.missing_vars.join(", "));
            }
            Ok(data.expanded)
        };
        let executable=expand(&server.command)?;
        let args=server.args.iter().map(|v|expand(v)).collect::<Result<Vec<_>>>()?;
        let mut env=std::collections::HashMap::new();
        for (k,v) in &server.env {env.insert(k.clone(),expand(v)?);}
        let mut process = Command::new(&executable);
        process.args(&args).envs(env).stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true);
        let mut child = process.spawn()
            .with_context(|| format!("Starting MCP server {}", server.id))?;
        let input = child.stdin.take().context("MCP stdin unavailable")?;
        let output = BufReader::new(child.stdout.take().context("MCP stdout unavailable")?);
        let mut client = Self { name: server.id.clone(), _child: child, input, output, id: 1, server_info: None, tools_changed: false };
        let initialized=timeout(RPC_TIMEOUT, client.request("initialize", json!({
            "protocolVersion":"2025-03-26",
            "capabilities":{},
            "clientInfo":{"name":"atlas-agent","version":env!("CARGO_PKG_VERSION")}
        }))).await.context("MCP initialize timed out")??;
        client.server_info=initialized.get("serverInfo").cloned();
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
            let method=message.get("method").and_then(Value::as_str);
            if method==Some("notifications/tools/list_changed"){
                self.tools_changed=true;
            }
            if message.get("id").is_some() && method.is_some(){
                if method==Some("ping"){
                    self.send(&json!({"jsonrpc":"2.0","id":message["id"],
                        "result":{}})).await?;
                }else{
                    self.send(&json!({"jsonrpc":"2.0","id":message["id"],
                        "error":{"code":-32601,"message":"Unsupported client method"}})).await?;
                }
            }
        }
    }
}

enum TransportClient {
    Stdio(Client),
    Http(crate::mcp_http::HttpClient),
}
impl TransportClient {
    async fn connect(config:&McpServerConfig)->Result<Self>{
        match config.transport.as_deref().unwrap_or("stdio"){
            "stdio"=>Ok(Self::Stdio(Client::connect(config).await?)),
            "http"=>Ok(Self::Http(crate::mcp_http::HttpClient::connect(config).await?)),
            other=>bail!("Unsupported MCP transport: {other}"),
        }
    }
    async fn request(&mut self,method:&str,params:Value)->Result<Value>{
        match self{
            Self::Stdio(c)=>c.request(method,params).await,
            Self::Http(c)=>c.request(method,params).await,
        }
    }
    fn take_tools_changed(&mut self)->bool{
        match self{
            Self::Stdio(c)=>std::mem::take(&mut c.tools_changed),
            Self::Http(_)=>false,
        }
    }
    fn mark_tools_changed(&mut self){
        if let Self::Stdio(c)=self{c.tools_changed=true}
    }
    fn server_info(&self)->Option<Value>{
        match self{
            Self::Stdio(c)=>c.server_info.clone(),
            Self::Http(c)=>c.server_info.clone(),
        }
    }
    fn failed_process(&mut self)->Result<Option<String>>{
        match self {
            Self::Stdio(c)=>Ok(c._child.try_wait()?.map(|status|
                format!("MCP process exited: {status}"))),
            Self::Http(_)=>Ok(None),
        }
    }
}
pub struct McpManager {
    clients: Vec<TransportClient>,
    tools: Vec<Value>,
    routes: HashMap<String, (usize, String)>,
    server_statuses: Vec<Value>,
    status_clients: Vec<Option<usize>>,
}

impl McpManager {
    pub async fn connect(servers: &[McpServerConfig])->Result<Self>{
        if servers.len()>16 {bail!("Too many MCP servers")}
        let mut out=Self{clients:Vec::new(),tools:Vec::new(),
            routes:HashMap::new(),server_statuses:Vec::new(),status_clients:Vec::new()};
        for server in servers{
            // Failed servers are isolated; one broken MCP process must not
            // prevent the healthy tools from operating.
            let mut client=match TransportClient::connect(server).await{
                Ok(client)=>client,
                Err(error)=>{
                    out.failed_status(server,format!("Connection failed: {error:#}"));
                    continue;
                }
            };
            let mut discovered=Vec::new();
            let mut display_tools=Vec::new();
            let mut cursor:Option<String>=None;
            let mut seen_cursors=std::collections::HashSet::new();
            let discovery:Result<()> = async {
                for _ in 0..20 {
                    let params=cursor.as_ref().map_or(json!({}),|c|json!({"cursor":c}));
                    let page=timeout(RPC_TIMEOUT,client.request("tools/list",params))
                        .await.context("MCP discovery timed out")??;
                    let tools=page.get("tools").and_then(Value::as_array)
                        .context("MCP tools/list missing tools array")?;
                    for tool in tools {
                        let name=tool.get("name").and_then(Value::as_str)
                            .context("MCP tool missing name")?;
                        let alias=format!("mcp_{}_{}",
                            runtime_contracts::normalize_mcp_name(&server.id)
                                .chars().take(45).collect::<String>(),
                            out.routes.len()+discovered.len());
                        let desc=tool.get("description").and_then(Value::as_str)
                            .unwrap_or(name);
                        let params=tool.get("inputSchema").cloned()
                            .unwrap_or(json!({"type":"object","properties":{}}));
                        discovered.push((alias,name.to_owned(),desc.to_owned(),params));
                        display_tools.push(json!({"name":name,"description":desc}));
                    }
                    cursor=page.get("nextCursor").and_then(Value::as_str).map(str::to_owned);
                    match cursor.as_ref(){
                        None=>break,
                        Some(c) if !seen_cursors.insert(c.clone())=>
                            bail!("MCP repeated pagination cursor"),
                        _=>{}
                    }
                }
                if cursor.is_some(){bail!("MCP exceeded 20 discovery pages")}
                Ok(())
            }.await;
            if let Err(error)=discovery{
                out.failed_status(server,format!("Tool discovery failed: {error:#}"));
                continue;
            }
            let client_index=out.clients.len();
            let server_info=client.server_info();
            for (alias,name,desc,params) in discovered {
                out.tools.push(json!({"type":"function","function":{
                    "name":alias,"description":format!("MCP {}: {desc}. Requires approval.",server.id),
                    "parameters":params
                }}));
                out.routes.insert(alias,(client_index,name));
            }
            out.server_statuses.push(json!({
                "name":server.id,"status":"connected","serverInfo":server_info,
                "config":Self::public_config(server),"tools":display_tools
            }));
            out.status_clients.push(Some(client_index));
            out.clients.push(client);
        }
        Ok(out)
    }
    fn public_config(server:&McpServerConfig)->Value {
        match server.transport.as_deref().unwrap_or("stdio"){
            "stdio"=>json!({"type":"stdio","command":server.command,"args":server.args}),
            _=>{
                // URL query parameters can contain access tokens. SDK status
                // must not reflect secrets even when configured explicitly.
                let url=server.url.as_deref().and_then(|s|reqwest::Url::parse(s).ok())
                    .map(|mut u|{u.set_query(None);u.set_fragment(None);u.to_string()});
                json!({"type":"http","url":url})
            }
        }
    }
    fn failed_status(&mut self,server:&McpServerConfig,message:String){
        let mut error=message;
        if let Some(url)=&server.url {
            error=error.replace(url,"[redacted MCP endpoint]");
        }
        for value in server.headers.values() {
            if !value.is_empty(){error=error.replace(value,"[redacted]");}
        }
        self.server_statuses.push(json!({"name":server.id,"status":"failed",
            "config":Self::public_config(server),"error":error}));
        self.status_clients.push(None);
    }

    /// Refresh MCP tools only after the server sends the standard
    /// tools/list_changed notification. Discovery is transactional; failed
    /// refresh keeps the last working tool routes and is retried later.
    pub async fn refresh_changed_tools(&mut self)->Result<()>{
        for client_index in 0..self.clients.len(){
            if !self.clients[client_index].take_tools_changed(){continue}
            let status_index=self.status_clients.iter().position(|v|
                *v==Some(client_index)).context("MCP status index unavailable")?;
            let server_name=self.server_statuses[status_index]["name"].as_str()
                .context("MCP server name missing")?.to_owned();
            let mut cursor:Option<String>=None;
            let mut seen=std::collections::HashSet::new();
            let mut discovered=Vec::new();
            let fetch:Result<()> = async {
                for _ in 0..20{
                    let params=cursor.as_ref().map_or(json!({}),|c|json!({"cursor":c}));
                    let reply=timeout(RPC_TIMEOUT,self.clients[client_index]
                        .request("tools/list",params))
                        .await.context("MCP dynamic discovery timed out")??;
                    let list=reply.get("tools").and_then(Value::as_array)
                        .context("MCP tools/list missing tools")?;
                    for tool in list{
                        let real=tool.get("name").and_then(Value::as_str)
                            .context("MCP dynamic tool missing name")?.to_owned();
                        let description=tool.get("description").and_then(Value::as_str)
                            .unwrap_or(&real).to_owned();
                        let schema=tool.get("inputSchema").cloned()
                            .unwrap_or(json!({"type":"object","properties":{}}));
                        discovered.push((real,description,schema));
                    }
                    cursor=reply.get("nextCursor").and_then(Value::as_str).map(str::to_owned);
                    match &cursor{
                        None=>break,
                        Some(c) if !seen.insert(c.clone())=>bail!("MCP pagination cursor repeated"),
                        _=>{}
                    }
                }
                if cursor.is_some(){bail!("MCP dynamic discovery exceeded 20 pages")}
                Ok(())
            }.await;
            if let Err(error)=fetch{
                self.clients[client_index].mark_tools_changed();
                return Err(error);
            }
            let removed=self.routes.iter().filter_map(|(alias,(index,_))|
                (*index==client_index).then_some(alias.clone())).collect::<Vec<_>>();
            for alias in &removed{self.routes.remove(alias);}
            self.tools.retain(|tool|!removed.iter().any(|alias|
                tool.pointer("/function/name").and_then(Value::as_str)==Some(alias.as_str())));
            let mut listed=Vec::new();
            for (real,description,schema) in discovered{
                let normalized=runtime_contracts::normalize_mcp_name(&server_name);
                let mut n=self.routes.len();
                let alias=loop{
                    let candidate=format!("mcp_{}_{}",
                        normalized.chars().take(45).collect::<String>(),n);
                    if !self.routes.contains_key(&candidate){break candidate}
                    n+=1;
                };
                self.routes.insert(alias.clone(),(client_index,real.clone()));
                self.tools.push(json!({"type":"function","function":{
                    "name":alias,"description":format!(
                        "MCP {server_name}: {description}. Requires approval."),
                    "parameters":schema
                }}));
                listed.push(json!({"name":real,"description":description}));
            }
            self.server_statuses[status_index]["tools"]=json!(listed);
        }
        Ok(())
    }

    /// SDK status comes from the live managed process, not the configured name.
    /// Do not expose environment variables or secrets in the SDK response.
    pub fn statuses(&mut self)->Result<Vec<Value>>{
        let mut statuses=self.server_statuses.clone();
        for (status,index) in statuses.iter_mut().zip(&self.status_clients){
            let Some(index)=index else{continue};
            if let Some(error)=self.clients[*index].failed_process()?{
                status["status"]=json!("failed");
                status["error"]=json!(error);
                if let Some(fields)=status.as_object_mut(){fields.remove("tools");}
            }
        }
        Ok(statuses)
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
        let args:Value=serde_json::from_str(&call.function.arguments)?;
        if !args.is_object(){bail!("MCP tool arguments must be an object")}
        let response = timeout(RPC_TIMEOUT, self.clients[index].request("tools/call",
            json!({"name":real_name,"arguments":args})))
            .await.context("MCP tool call timed out")??;
        // Keep non-text blocks and structured data intact: silently dropping
        // image/resource content corrupts the tool response seen by the model.
        let parts=response.get("content").and_then(Value::as_array);
        let only_text=parts.is_some_and(|items|items.iter().all(|item|
            item.get("type").and_then(Value::as_str)==Some("text") &&
            item.get("text").and_then(Value::as_str).is_some()));
        let result=if only_text && response.get("structuredContent").is_none(){
            let texts=parts.unwrap().iter().filter_map(|item|item["text"].as_str())
                .collect::<Vec<_>>().join("\n");
            if response["isError"]==true {format!("ERROR: {texts}")} else {texts}
        }else if response["isError"]==true{
            format!("ERROR: {}",response)
        }else{response.to_string()};
        Ok(result.chars().take(16_000).collect())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::protocol::FunctionCall;

    #[tokio::test]
    async fn live_tool_list_changed_notification_refreshes_model_tool_registry(){
        let script=r#"while IFS= read -r line; do
case "$line" in
  *'"method":"initialize"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"swap"}}}'
    ;;
  *'"id":2'*'"method":"tools/list"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"before","inputSchema":{"type":"object"}}]}}'
    ;;
  *'"id":4'*'"method":"tools/list"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":4,"result":{"tools":[{"name":"after","inputSchema":{"type":"object","properties":{"next":{"type":"string"}}}}]}}'
    ;;
  *'"id":3'*'"method":"tools/call"'*)
    printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}'
    printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"before ok"}]}}'
    ;;
  *'"id":5'*'"method":"tools/call"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"after ok"}]}}'
    ;;
esac
done"#;
        let config=McpServerConfig{id:"swap".into(),command:"sh".into(),
            args:vec!["-c".into(),script.into()],env:HashMap::new(),
            transport:None,url:None,headers:HashMap::new()};
        let mut mgr=McpManager::connect(&[config]).await.unwrap();
        assert_eq!(mgr.statuses().unwrap()[0]["tools"][0]["name"],"before");
        let call=|name:&str,id:&str|ToolCall{id:id.into(),kind:"function".into(),
            function:FunctionCall{name:name.into(),arguments:"{}".into()}};
        assert_eq!(mgr.execute(&call("mcp_swap_0","1")).await,"before ok");
        mgr.refresh_changed_tools().await.unwrap();
        assert_eq!(mgr.statuses().unwrap()[0]["tools"][0]["name"],"after");
        assert_eq!(mgr.definitions().len(),1);
        assert_eq!(mgr.definitions()[0]["function"]["parameters"]["properties"]["next"]["type"],"string");
        assert_eq!(mgr.execute(&call("mcp_swap_0","2")).await,"after ok");
    }
    #[tokio::test]
    async fn failed_stdio_server_cannot_break_healthy_server_and_indexing(){
        let script=r#"while IFS= read -r line; do
case "$line" in
  *'"method":"initialize"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"healthy"}}}'
    ;;
  *'"method":"tools/list"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","inputSchema":{"type":"object"}}]}}'
    ;;
  *'"method":"tools/call"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"healthy response"}]}}'
    ;;
esac
done"#;
        let make=|id:&str,command:&str,args:Vec<String>|McpServerConfig{
            id:id.into(),command:command.into(),args,env:HashMap::new(),
            transport:None,url:None,headers:HashMap::new()
        };
        let broken=make("broken","nonexistent_mcp_executable_for_test",vec![]);
        let healthy=make("healthy","sh",vec!["-c".into(),script.into()]);
        let mut mgr=McpManager::connect(&[broken,healthy]).await.unwrap();
        assert_eq!(mgr.definitions().len(),1);
        assert!(mgr.handles("mcp_healthy_0"));
        let states=mgr.statuses().unwrap();
        assert_eq!(states.len(),2);
        assert_eq!(states[0]["status"],"failed");
        assert_eq!(states[1]["status"],"connected");
        assert_eq!(states[1]["tools"][0]["name"],"echo");
        let call=ToolCall{id:"ok".into(),kind:"function".into(),
            function:FunctionCall{name:"mcp_healthy_0".into(),
                arguments:"{}".into()}};
        assert_eq!(mgr.execute(&call).await,"healthy response");
    }
    #[tokio::test]
    async fn failed_discovery_does_not_publish_half_registered_tools(){
        let script=r#"while IFS= read -r line; do
case "$line" in
  *'"method":"initialize"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"broken"}}}'
    ;;
  *'"method":"tools/list"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"first","inputSchema":{"type":"object"}},{"wrong":"missing name"}]}}'
    ;;
esac
done"#;
        let config=McpServerConfig{id:"partial".into(),command:"sh".into(),
            args:vec!["-c".into(),script.into()],env:HashMap::new(),
            transport:None,url:None,headers:HashMap::new()};
        let mut mgr=McpManager::connect(&[config]).await.unwrap();
        assert!(mgr.definitions().is_empty());
        assert!(!mgr.handles("mcp_partial_0"));
        let states=mgr.statuses().unwrap();
        assert_eq!(states.len(),1);
        assert_eq!(states[0]["status"],"failed");
    }
    #[tokio::test]
    async fn mcp_tool_error_and_structured_payload_survive_model_output(){
        let script=r#"while IFS= read -r line; do
case "$line" in
  *'"method":"initialize"'*) echo '{"jsonrpc":"2.0","id":1,"result":{}}' ;;
  *'"method":"tools/list"'*) echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo"}]}}' ;;
  *'"method":"tools/call"'*) echo '{"jsonrpc":"2.0","id":3,"result":{"isError":true,"content":[{"type":"text","text":"permission denied"}],"structuredContent":{"code":"denied"}}}' ;;
esac
done"#;
        let c=McpServerConfig{id:"error".into(),command:"sh".into(),
            args:vec!["-c".into(),script.into()],env:HashMap::new(),
            transport:None,url:None,headers:HashMap::new()};
        let mut mgr=McpManager::connect(&[c]).await.unwrap();
        let call=ToolCall{id:"bad".into(),kind:"function".into(),
            function:FunctionCall{name:"mcp_error_0".into(),
                arguments:"{}".into()}};
        let result=mgr.execute(&call).await;
        assert!(result.starts_with("ERROR:"));
        assert!(result.contains("permission denied"));
        assert!(result.contains("structuredContent"));
        let illegal=ToolCall{id:"illegal".into(),kind:"function".into(),
            function:FunctionCall{name:"mcp_error_0".into(),
                arguments:"[]".into()}};
        assert!(mgr.execute(&illegal).await.contains("must be an object"));
    }
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
            env: std::collections::HashMap::new(),
            transport:None,url:None,headers:std::collections::HashMap::new(),
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
        let online=manager.statuses().unwrap();
        assert_eq!(online[0]["name"],"mock");
        assert_eq!(online[0]["status"],"connected");
        assert_eq!(online[0]["serverInfo"]["name"],"mock");
        assert_eq!(online[0]["tools"][0]["name"],"echo");
        assert!(online[0]["config"].get("env").is_none());
        if let TransportClient::Stdio(client)=&mut manager.clients[0]{
            client._child.kill().await.unwrap();
        }else{panic!("expected stdio client")}
        let offline=manager.statuses().unwrap();
        assert_eq!(offline[0]["status"],"failed");
        assert!(offline[0].get("tools").is_none());
    }
}


#[cfg(test)]
mod remote_tests {
    use super::*;
    use crate::protocol::FunctionCall;
    use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener};

    async fn take_http(stream:&mut tokio::net::TcpStream)->String{
        let mut bytes=Vec::new();let mut buf=[0u8;4096];
        loop {
            let read=stream.read(&mut buf).await.unwrap();
            if read==0{break}
            bytes.extend_from_slice(&buf[..read]);
            if let Some(pos)=bytes.windows(4).position(|s|s==b"\r\n\r\n"){
                let head=String::from_utf8_lossy(&bytes[..pos]).to_ascii_lowercase();
                let length=head.lines().find_map(|line|
                    line.strip_prefix("content-length:")
                        .and_then(|v|v.trim().parse::<usize>().ok())).unwrap_or(0);
                if bytes.len()>=pos+4+length{break}
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
    #[tokio::test]
    async fn generic_http_mcp_negotiates_session_sse_discovery_and_tool_dispatch(){
        let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address=listener.local_addr().unwrap();
        let server=tokio::spawn(async move {
            let mut requests=Vec::new();
            for n in 0..4{
                let (mut conn,_)=listener.accept().await.unwrap();
                let request=take_http(&mut conn).await;
                requests.push(request);
                let (type_name,body)=match n{
                    0=>("application/json",json!({"jsonrpc":"2.0","id":1,
                        "result":{"protocolVersion":"2025-03-26",
                            "serverInfo":{"name":"mock-http","version":"1"},
                            "capabilities":{"tools":{}}}}).to_string()),
                    1=>("application/json",String::new()),
                    2=>("text/event-stream",
                        "event: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\r\n\
                         data: \"result\":{\"tools\":[{\"name\":\"remote_echo\",\
                         \"description\":\"remote text\",\
                         \"inputSchema\":{\"type\":\"object\",\"properties\":{}}}]}}\r\n\r\n"
                         .to_owned()),
                    _=>("application/json",json!({"jsonrpc":"2.0","id":3,
                        "result":{"content":[{"type":"text","text":"remote: ok"}]}})
                         .to_string())
                };
                let reply=format!("HTTP/1.1 {} OK\r\nContent-Type: {type_name}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{body}",
                    if n==1{202}else{200},body.len(),
                    if n==0{"Mcp-Session-Id: local-123\r\n"}else{""});
                conn.write_all(reply.as_bytes()).await.unwrap();
                conn.shutdown().await.unwrap();
            }
            requests
        });
        let config=McpServerConfig{id:"remote".into(),command:String::new(),
            args:vec![],env:HashMap::new(),transport:Some("http".into()),
            url:Some(format!("http://{address}/mcp")),headers:HashMap::new()};
        let mut mgr=McpManager::connect(&[config]).await.unwrap();
        assert!(mgr.handles("mcp_remote_0"));
        assert_eq!(mgr.statuses().unwrap()[0]["config"]["type"],"http");
        assert_eq!(mgr.statuses().unwrap()[0]["serverInfo"]["name"],"mock-http");
        let call=ToolCall{id:"tool-call".into(),kind:"function".into(),
            function:FunctionCall{name:"mcp_remote_0".into(),
                arguments:json!({"message":"ok"}).to_string()}};
        assert_eq!(mgr.execute(&call).await,"remote: ok");
        let seen=server.await.unwrap();
        assert!(seen[0].to_ascii_lowercase()
            .contains("accept: application/json, text/event-stream"));
        assert!(seen[1].to_ascii_lowercase().contains("mcp-session-id: local-123"));
        assert!(seen[2].to_ascii_lowercase()
            .contains("mcp-protocol-version: 2025-03-26"));
        assert!(seen[3].contains("tools/call"));
        assert!(seen[3].contains("remote_echo"));
    }
}
