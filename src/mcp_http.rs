//! Generic MCP Streamable HTTP transport (2025-03-26).
//! Requests are individually timed; session headers are never logged.
use crate::{compat_helpers,config::McpServerConfig};
use anyhow::{bail,Context,Result};
use reqwest::{Client,Url,header::{HeaderMap,HeaderName,HeaderValue,CONTENT_TYPE,ACCEPT}};
use serde_json::{json,Value};
use std::{str::FromStr,time::Duration};
pub struct HttpClient {
    name:String,
    client:Client,
    url:Url,
    headers:HeaderMap,
    session:Option<HeaderValue>,
    id:u64,
    pub server_info:Option<Value>,
    pub capabilities:Option<Value>,
    pub tools_changed:bool,
    pub resource_updates:Vec<Value>,
}
impl HttpClient{
    pub async fn connect(config:&McpServerConfig)->Result<Self>{
        let url=config.url.as_deref().context("HTTP MCP requires url")?;
        let url=Url::parse(url).context("Invalid MCP URL")?;
        if !matches!(url.scheme(),"http"|"https"){bail!("MCP HTTP requires http(s)")}
        if !url.username().is_empty()||url.password().is_some(){
            bail!("MCP URL must not contain credentials");
        }
        let mut headers=HeaderMap::new();
        for (key,value) in &config.headers{
            let key=HeaderName::from_str(key).context("Invalid MCP header name")?;
            if matches!(key.as_str(),"accept"|"content-type"|"mcp-session-id"|
                "mcp-protocol-version"|"host"){
                bail!("MCP reserved header cannot be overridden: {key}");
            }
            let expanded=compat_helpers::expand_env_vars_in_string(value);
            if !expanded.missing_vars.is_empty(){
                bail!("MCP {} missing header environment variables: {}",
                    config.id,expanded.missing_vars.join(", "));
            }
            headers.insert(key,HeaderValue::from_str(&expanded.expanded)
                .context("Invalid MCP header value")?);
        }
        let client=Client::builder().timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none()).build()?;
        let mut http=Self{name:config.id.clone(),client,url,headers,
            session:None,id:1,server_info:None,capabilities:None,
            tools_changed:false,resource_updates:Vec::new()};
        let initial=http.request("initialize",json!({
            "protocolVersion":"2025-03-26","capabilities":{},
            "clientInfo":{"name":"atlas-agent","version":env!("CARGO_PKG_VERSION")}
        })).await?;
        http.server_info=initial.get("serverInfo").cloned();
        http.capabilities=initial.get("capabilities").cloned();
        http.notify("notifications/initialized",json!({})).await?;
        Ok(http)
    }
    async fn post(&mut self,request:&Value,expect_response:bool)->Result<Option<Value>>{
        let mut headers=self.headers.clone();
        headers.insert(CONTENT_TYPE,HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT,HeaderValue::from_static("application/json, text/event-stream"));
        headers.insert(HeaderName::from_static("mcp-protocol-version"),
            HeaderValue::from_static("2025-03-26"));
        if let Some(session)=&self.session{
            headers.insert(HeaderName::from_static("mcp-session-id"),session.clone());
        }
        let mut reply=self.client.post(self.url.clone()).headers(headers)
            .json(request).send().await.with_context(||
                format!("HTTP MCP {} request failed",self.name))?;
        if reply.status().is_redirection(){bail!("MCP redirect blocked")}
        if reply.status()==reqwest::StatusCode::NOT_FOUND&&self.session.is_some(){
            bail!("MCP session expired; reconnect required");
        }
        if !reply.status().is_success(){
            bail!("MCP {} server responded {}",self.name,reply.status())
        }
        if self.session.is_none(){
            if let Some(id)=reply.headers().get("mcp-session-id"){
                if id.as_bytes().is_empty()||id.as_bytes().len()>512{
                    bail!("MCP session ID has invalid length")
                }
                self.session=Some(id.clone());
            }
        }
        if !expect_response{return Ok(None)}
        if reply.status()==reqwest::StatusCode::ACCEPTED{
            bail!("MCP request received 202 without JSON-RPC response");
        }
        let type_header=reply.headers().get(CONTENT_TYPE).and_then(|v|v.to_str().ok())
            .unwrap_or("").to_ascii_lowercase();
        if type_header.starts_with("text/event-stream"){
            // Streamable HTTP servers may keep this response open after the
            // requested result. Never wait for EOF: that deadlocks tools/call
            // and hides notifications delivered ahead of the result.
            let id=request.get("id").context("MCP request is missing id")?;
            let mut pending=Vec::new();
            let mut received=0usize;
            while let Some(chunk)=reply.chunk().await?{
                received=received.checked_add(chunk.len())
                    .context("MCP SSE response length overflow")?;
                if received>2*1024*1024{bail!("MCP HTTP response exceeds 2 MB")}
                pending.extend_from_slice(&chunk);
                while let Some((end,separator_len))=sse_event_boundary(&pending){
                    let frame=std::str::from_utf8(&pending[..end])
                        .context("Invalid MCP SSE UTF-8")?;
                    for event in parse_sse(frame)?{
                        let kind=event.get("method").and_then(Value::as_str);
                        if kind==Some("notifications/tools/list_changed"){
                            self.tools_changed=true;
                        }
                        if matches!(kind,Some("notifications/resources/updated"|
                            "notifications/resources/list_changed")) &&
                            self.resource_updates.len()<1024{
                            self.resource_updates.push(event.clone());
                        }
                        if event.get("id")==Some(id){
                            return Ok(Some(event));
                        }
                    }
                    pending.drain(..end+separator_len);
                }
            }
            bail!("MCP SSE stream ended without matching RPC response")
        }
        if type_header.starts_with("application/json"){
            let bytes=reply.bytes().await?;
            if bytes.len()>2*1024*1024{bail!("MCP HTTP response exceeds 2 MB")}
            return Ok(Some(serde_json::from_slice(&bytes)
                .context("Invalid MCP JSON response")?))
        }
        bail!("MCP HTTP response has unsupported content type")
    }
    pub async fn notify(&mut self,method:&str,params:Value)->Result<()>{
        self.post(&json!({"jsonrpc":"2.0","method":method,"params":params}),false).await?;
        Ok(())
    }
    pub async fn request(&mut self,method:&str,params:Value)->Result<Value>{
        let id=self.id;self.id=self.id.checked_add(1).context("MCP ID overflow")?;
        let response=self.post(&json!({"jsonrpc":"2.0","id":id,
            "method":method,"params":params}),true).await?
            .context("MCP response was empty")?;
        if response.get("jsonrpc").and_then(Value::as_str)!=Some("2.0")||
            response.get("id").and_then(Value::as_u64)!=Some(id){
            bail!("MCP JSON-RPC response version or ID mismatch");
        }
        if let Some(error)=response.get("error"){bail!("MCP server error: {error}")}
        response.get("result").cloned().context("MCP response missing result")
    }
}
// Locate complete SSE events without decoding incomplete UTF-8 chunks.
// CRLF can be split across HTTP chunks, so only complete separators count.
fn sse_event_boundary(bytes:&[u8])->Option<(usize,usize)>{
    for index in 0..bytes.len(){
        if bytes[index..].starts_with(b"\r\n\r\n"){return Some((index,4))}
        if bytes[index..].starts_with(b"\n\n"){return Some((index,2))}
    }
    None
}
fn parse_sse(text:&str)->Result<Vec<Value>>{
    let mut results=Vec::new();let mut data=Vec::new();let mut kind=String::new();
    for line in text.lines().chain(std::iter::once("")){
        let line=line.trim_end_matches('\r');
        if line.is_empty(){
            if !data.is_empty()&&(kind.is_empty()||kind=="message"){
                results.push(serde_json::from_str(&data.join("\n"))
                    .context("Invalid JSON in MCP SSE event")?);
            }
            data.clear();kind.clear();continue;
        }
        if let Some(value)=line.strip_prefix("data:"){
            data.push(value.strip_prefix(' ').unwrap_or(value).to_owned())
        }else if let Some(value)=line.strip_prefix("event:"){
            kind=value.trim().to_owned()
        }
    }
    Ok(results)
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::collections::HashMap;
    fn config(url:String)->McpServerConfig{
        McpServerConfig{id:"remote".into(),command:String::new(),args:Vec::new(),
            env:HashMap::new(),transport:Some("http".into()),url:Some(url),
            headers:HashMap::new()}
    }
    #[tokio::test]
    async fn http_sse_resource_update_before_rpc_response_survives_and_does_not_wait_for_eof(){
        use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener};
        let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address=listener.local_addr().unwrap();
        let server=tokio::spawn(async move{
            let mut observed=Vec::new();
            for n in 0..4{
                let (mut connection,_)=listener.accept().await.unwrap();
                let mut input=Vec::new();let mut buf=[0u8;4096];
                loop{
                    let count=connection.read(&mut buf).await.unwrap();
                    assert!(count>0);
                    input.extend_from_slice(&buf[..count]);
                    if let Some(split)=input.windows(4).position(|b|b==b"\r\n\r\n"){
                        let header=String::from_utf8_lossy(&input[..split]);
                        let len=header.lines().find_map(|line|line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value|value.trim().parse::<usize>().ok())).unwrap_or(0);
                        if input.len()>=split+4+len{break}
                    }
                }
                observed.push(String::from_utf8_lossy(&input).into_owned());
                let (kind,body)=match n{
                    0=>("application/json",json!({"jsonrpc":"2.0","id":1,
                        "result":{"capabilities":{"resources":{"subscribe":true}}}}).to_string()),
                    1=>("application/json",String::new()),
                    2=>("text/event-stream",format!(
                        "event: message\r\ndata: {}\r\n\r\n\
                        event: message\r\ndata: {}\r\n\r\n",
                        json!({"jsonrpc":"2.0","method":"notifications/resources/updated",
                            "params":{"uri":"file:///image","revision":3}}),
                        json!({"jsonrpc":"2.0","id":2,"result":{}})
                    )),
                    _=>("application/json",json!({"jsonrpc":"2.0","id":3,
                        "result":{}}).to_string())
                };
                let header=format!("HTTP/1.1 {} OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    if n==1{202}else{200},body.len());
                connection.write_all(header.as_bytes()).await.unwrap();
                if !body.is_empty(){
                    // Deliberately split the JSON payload across TCP reads.
                    let mid=body.len()/2;
                    connection.write_all(&body.as_bytes()[..mid]).await.unwrap();
                    connection.write_all(&body.as_bytes()[mid..]).await.unwrap();
                }
                connection.shutdown().await.unwrap();
            }
            observed
        });
        let mut http=HttpClient::connect(&config(format!("http://{address}/mcp")))
            .await.unwrap();
        assert_eq!(http.capabilities.as_ref().unwrap()["resources"]["subscribe"],true);
        http.request("resources/subscribe",json!({"uri":"file:///image"}))
            .await.unwrap();
        assert_eq!(http.resource_updates.len(),1);
        assert_eq!(http.resource_updates[0]["params"]["revision"],3);
        assert_eq!(http.resource_updates[0]["method"],"notifications/resources/updated");
        http.request("resources/unsubscribe",json!({"uri":"file:///image"}))
            .await.unwrap();
        let requests=server.await.unwrap();
        assert!(requests[2].contains("resources/subscribe"));
        assert!(requests[3].contains("resources/unsubscribe"));
    }
    #[test]
    fn stream_parser_skips_keepalives_and_nonmessage_events(){
        let stream=": keepalive\r\nevent: update\r\ndata: {}\r\n\r\n\
            event: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\r\n\
            data: \"result\":{}}\r\n\r\n";
        let events=parse_sse(stream).unwrap();
        assert_eq!(events.len(),1);
        assert_eq!(events[0]["id"],3);
    }
    #[tokio::test]
    async fn rejects_url_credentials_and_reserved_headers_without_network(){
        let mut c=config("http://user:password@127.0.0.1:1/mcp".into());
        assert!(format!("{:#}",HttpClient::connect(&c).await.err().unwrap())
            .contains("credentials"));
        c.url=Some("http://127.0.0.1:1/mcp".into());
        c.headers.insert("Mcp-Session-Id".into(),"forged".into());
        assert!(format!("{:#}",HttpClient::connect(&c).await.err().unwrap())
            .contains("reserved header"));
    }
}
