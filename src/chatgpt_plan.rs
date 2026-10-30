//! OpenAI's documented Sign in with ChatGPT flow for open-source/local apps.
//! https://developers.openai.com/siwc/token-sharing-open-source/
//! ChatGPT-plan inference uses the public Responses API, never backend-api.
use crate::{
    config::{ModelTuning, ProviderProfile},
    oauth_crypto::{generate_code_challenge, generate_code_verifier, generate_state},
    plain_text_storage::PlainTextStorage,
    protocol::{FunctionCall, Message, ModelRequest, ModelResponse, TokenUsage, ToolCall},
    secure_fallback::SecureStorage,
};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use clap::Subcommand;
use futures::StreamExt;
use reqwest::{Client, Url};
use ring::signature::{RsaPublicKeyComponents, RSA_PKCS1_2048_8192_SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::IpAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc::UnboundedSender,
};

const AUTH: &str = "https://auth.openai.com";
const RESOURCE: &str = "https://api.openai.com/v1";
const PERMISSION: &str = "chatgpt.tokens.use.direct";
/// Internal, versioned envelope for API-owned encrypted reasoning items.
pub const REASONING_ITEMS_PREFIX: &str = "__atlas_opaque_responses_items_v1__:";

#[derive(Debug, Clone, Subcommand)]
pub enum AccountAction {
    /// Sign in using system browser, OAuth PKCE and local loopback callback.
    Login,
    /// List visible account models using the current OAuth access token.
    Models,
    /// Show local sign-in and plan-permission status without exposing tokens.
    Status,
    /// Revoke the selected session and clear local token credentials.
    Logout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Stored {
    host_id: String,
    #[serde(default)]
    session: Option<Session>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Session {
    client_id: String,
    subject: String,
    email: Option<String>,
    access_token: String,
    refresh_token: String,
    id_token: String,
    scopes: Vec<String>,
    expires_at: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    scope: String,
    expires_in: u64,
    #[serde(default)]
    earliest_refresh_at: Option<u64>,
}

fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn storage() -> Result<PlainTextStorage> {
    Ok(PlainTextStorage::new(
        crate::config::Config::path()?.with_file_name(".chatgpt-plan.json"),
    ))
}

fn read_saved() -> Result<Option<Stored>> {
    let file = storage()?.path;
    if !file.exists() {
        return Ok(None);
    }
    let saved =
        serde_json::from_slice(&fs::read(file)?).context("Invalid local ChatGPT credentials")?;
    Ok(Some(saved))
}

fn write_saved(saved: &Stored) -> Result<()> {
    if !storage()?.update(serde_json::to_value(saved)?).success {
        bail!("Failed to save ChatGPT credentials");
    }
    Ok(())
}

fn new_client() -> Result<Client> {
    Ok(crate::tls_ca::client_builder()?
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

fn inference_client() -> Result<Client> {
    Ok(crate::tls_ca::client_builder()?
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

fn scope_granted(scope: &str) -> bool {
    scope.split_whitespace().any(|item| item == PERMISSION)
}

/// Validate JWT signature and OIDC claims. Never trust an unverified token
/// to select a client_id, account, or ChatGPT-plan authorization.
async fn verify_id_token(
    client: &Client,
    id_token: &str,
    client_id: &str,
    nonce: Option<&str>,
) -> Result<(String, Option<String>)> {
    let parts = id_token.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        bail!("Invalid ID token segments")
    }
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0])?)?;
    if header.get("alg").and_then(Value::as_str) != Some("RS256") {
        bail!("Unsupported ID token signing algorithm");
    }
    let kid = header
        .get("kid")
        .and_then(Value::as_str)
        .context("ID token missing key ID")?;
    let config = client
        .get(format!("{AUTH}/.well-known/openid-configuration"))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    let jwks_uri = config
        .get("jwks_uri")
        .and_then(Value::as_str)
        .context("OpenAI discovery document has no JWKS URI")?;
    let jwks_url = Url::parse(jwks_uri)?;
    if jwks_url.scheme() != "https" || jwks_url.host_str() != Some("auth.openai.com") {
        bail!("Untrusted JWKS endpoint")
    }
    let jwks = client
        .get(jwks_url)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    let keys = jwks
        .get("keys")
        .and_then(Value::as_array)
        .context("Missing JWKS keys")?;
    let key = keys
        .iter()
        .find(|key| {
            key.get("kid").and_then(Value::as_str) == Some(kid)
                && key.get("kty").and_then(Value::as_str) == Some("RSA")
                && key
                    .get("use")
                    .and_then(Value::as_str)
                    .is_none_or(|use_| use_ == "sig")
        })
        .context("ID token verification key not found")?;
    let n = URL_SAFE_NO_PAD.decode(
        key.get("n")
            .and_then(Value::as_str)
            .context("JWKS missing modulus")?,
    )?;
    let e = URL_SAFE_NO_PAD.decode(
        key.get("e")
            .and_then(Value::as_str)
            .context("JWKS missing exponent")?,
    )?;
    let sig = URL_SAFE_NO_PAD.decode(parts[2])?;
    RsaPublicKeyComponents { n: &n, e: &e }
        .verify(
            &RSA_PKCS1_2048_8192_SHA256,
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
            &sig,
        )
        .map_err(|_| anyhow::anyhow!("Invalid OpenAI ID token signature"))?;
    let payload: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1])?)?;
    if payload.get("iss").and_then(Value::as_str) != Some(AUTH) {
        bail!("Invalid OpenAI ID token issuer")
    }
    let audience_valid = payload.get("aud").and_then(Value::as_str) == Some(client_id)
        || payload
            .get("aud")
            .and_then(Value::as_array)
            .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(client_id)));
    if !audience_valid {
        bail!("OpenAI ID token audience mismatch")
    }
    let current = now()?;
    if payload
        .get("exp")
        .and_then(Value::as_u64)
        .is_none_or(|exp| exp <= current)
    {
        bail!("OpenAI ID token expired")
    }
    if payload
        .get("nbf")
        .and_then(Value::as_u64)
        .is_some_and(|v| v > current + 30)
    {
        bail!("OpenAI ID token not yet valid")
    }
    if let Some(expected) = nonce {
        if payload.get("nonce").and_then(Value::as_str) != Some(expected) {
            bail!("OpenAI ID token nonce mismatch")
        }
    }
    let subject = payload
        .get("sub")
        .and_then(Value::as_str)
        .filter(|sub| !sub.is_empty())
        .context("OpenAI ID token has no subject")?;
    Ok((
        subject.to_owned(),
        payload
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_owned),
    ))
}

async fn receive_callback(listener: &TcpListener, state: &str) -> Result<(String, Option<String>)> {
    let receive = async {
        loop {
            let (mut connection, addr) = listener.accept().await?;
            if !matches!(addr.ip(), IpAddr::V4(ip) if ip.is_loopback()) {
                continue;
            }
            let mut buffer = [0u8; 4096];
            let mut bytes = Vec::new();
            loop {
                let n = connection.read(&mut buffer).await?;
                if n == 0 {
                    bail!("Browser callback closed before completing request")
                }
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() > 16384 {
                    bail!("Oversized OAuth browser callback")
                }
                if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            let line = String::from_utf8_lossy(&bytes);
            let first = line
                .lines()
                .next()
                .context("Missing callback request line")?;
            let url_path = first
                .strip_prefix("GET ")
                .and_then(|v| v.split_whitespace().next())
                .context("OAuth callback must use GET")?;
            let url = Url::parse(&format!("http://127.0.0.1{url_path}"))?;
            if url.path() != "/auth/callback" {
                connection
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                    .await?;
                continue;
            }
            let params = url
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            if params.get("state").map(|x| x.as_ref()) != Some(state) {
                connection
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                    .await?;
                bail!("OAuth state did not match requested sign-in")
            }
            if let Some(reason) = params.get("error") {
                bail!("ChatGPT sign-in declined: {reason}")
            }
            let code = params
                .get("code")
                .filter(|v| !v.is_empty())
                .context("OAuth callback has no authorization code")?
                .to_string();
            let client_id = params.get("client_id").map(ToString::to_string);
            connection.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 23\r\n\r\nAuthentication received").await?;
            return Ok((code, client_id));
        }
    };
    tokio::time::timeout(Duration::from_secs(300), receive)
        .await
        .context("ChatGPT sign-in timed out")?
}

async fn login() -> Result<()> {
    let mut saved = match read_saved()? {
        Some(existing) => existing,
        None => {
            let created = Stored {
                host_id: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
                session: None,
            };
            write_saved(&created)?;
            created
        }
    };
    let previous = saved.session.as_ref();
    let client_id = previous
        .map(|s| s.client_id.as_str())
        .unwrap_or("dynamic_agent_client");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let uri = format!(
        "http://127.0.0.1:{}/auth/callback",
        listener.local_addr()?.port()
    );
    let verifier = generate_code_verifier()?;
    let state = generate_state()?;
    let nonce = generate_state()?;
    let mut url = Url::parse(&format!("{AUTH}/api/accounts/authorize"))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", client_id);
        if previous.is_none() {
            query.append_pair("agent_name_hint", "Atlas Agent");
        }
        query.append_pair("ext_agent_host_id", &saved.host_id);
        if let Some(token) = previous {
            query.append_pair("id_token_hint", &token.id_token);
        }
        query.append_pair("response_type", "code");
        query.append_pair("redirect_uri", &uri);
        query.append_pair(
            "scope",
            "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct",
        );
        query.append_pair("resource", RESOURCE);
        query.append_pair("state", &state);
        query.append_pair("nonce", &nonce);
        query.append_pair("code_challenge_method", "S256");
        query.append_pair("code_challenge", &generate_code_challenge(&verifier));
    }
    // The URL may contain an ID-token hint: never print it in logs.
    if !crate::browser_launcher::open_browser(url.as_str()).await {
        bail!("Could not open browser for ChatGPT sign-in")
    }
    eprintln!("Complete ChatGPT authorization in the browser.");
    let (code, returned_id) = receive_callback(&listener, &state).await?;
    let issued = if previous.is_none() {
        returned_id.context("OpenAI dynamic registration did not return an issued client ID")?
    } else {
        if returned_id.as_deref().is_some_and(|id| id != client_id) {
            bail!("ChatGPT callback changed the registered client ID")
        }
        client_id.to_owned()
    };
    if issued == "dynamic_agent_client" {
        bail!("Dynamic client registration was not completed")
    }
    let http = new_client()?;
    let response = http
        .post(format!("{AUTH}/api/accounts/oauth/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", issued.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", uri.as_str()),
            ("resource", RESOURCE),
        ])
        .send()
        .await?
        .error_for_status()?;
    let tokens: TokenResponse = response.json().await?;
    if !scope_granted(&tokens.scope) {
        bail!("ChatGPT plan permission was not granted; existing credentials remain unchanged")
    }
    let id_token = tokens
        .id_token
        .context("OpenAI token response missing ID token")?;
    let refresh_token = tokens
        .refresh_token
        .context("OpenAI token response missing refresh token")?;
    let (subject, email) = verify_id_token(&http, &id_token, &issued, Some(&nonce)).await?;
    if previous.is_some_and(|previous| previous.subject != subject) {
        bail!("Reauthorization returned a different ChatGPT account")
    }
    saved.session = Some(Session {
        client_id: issued,
        subject,
        email,
        access_token: tokens.access_token,
        refresh_token,
        id_token,
        scopes: tokens.scope.split_whitespace().map(str::to_owned).collect(),
        expires_at: now()?.saturating_add(tokens.expires_in),
    });
    write_saved(&saved)?;
    println!("ChatGPT plan sign-in authorized. Model inference uses the official Responses API.");
    Ok(())
}

async fn active_token() -> Result<String> {
    let mut saved = read_saved()?.context("Sign in with: atlas-agent chatgpt login")?;
    let mut session = saved
        .session
        .clone()
        .context("Sign in with: atlas-agent chatgpt login")?;
    if !session.scopes.iter().any(|v| v == PERMISSION) {
        bail!("ChatGPT plan permission is not enabled")
    }
    if now()?.saturating_add(120) >= session.expires_at {
        let http = new_client()?;
        let result = http
            .post(format!("{AUTH}/api/accounts/oauth/token"))
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", session.client_id.as_str()),
                ("refresh_token", session.refresh_token.as_str()),
                ("resource", RESOURCE),
            ])
            .send()
            .await?
            .error_for_status()?;
        let tokens: TokenResponse = result.json().await?;
        if !scope_granted(&tokens.scope) {
            bail!("Refreshed ChatGPT token lacks plan usage authorization")
        }
        if let Some(id) = &tokens.id_token {
            let (sub, _) = verify_id_token(&http, id, &session.client_id, None).await?;
            if sub != session.subject {
                bail!("ChatGPT refresh account identity changed")
            }
            session.id_token = id.clone();
        }
        session.access_token = tokens.access_token;
        session.refresh_token = tokens
            .refresh_token
            .context("Refresh response missing rotating token")?;
        session.scopes = tokens.scope.split_whitespace().map(str::to_owned).collect();
        session.expires_at = now()?.saturating_add(tokens.expires_in);
        saved.session = Some(session.clone());
        write_saved(&saved)?;
    }
    Ok(session.access_token)
}

/// Current account-scoped models; never substitute an API-key catalog.
pub fn has_plan_session() -> Result<bool> {
    Ok(read_saved()?.is_some_and(|saved| saved.session.is_some()))
}

pub async fn model_list() -> Result<Value> {
    let token = active_token().await?;
    let response = new_client()?
        .get(format!("{RESOURCE}/models"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    let payload = response.bytes().await?;
    if payload.len() > 4 * 1024 * 1024 {
        bail!("ChatGPT model catalog exceeds 4 MiB")
    }
    let data: Value = serde_json::from_slice(&payload)?;
    if !data.get("models").is_some_and(Value::is_array) {
        bail!("ChatGPT account model catalog unavailable")
    }
    Ok(data)
}

pub async fn run(action: &AccountAction) -> Result<()> {
    match action {
        AccountAction::Login => login().await,
        AccountAction::Status => {
            let saved = read_saved()?;
            if let Some(Some(session)) = saved.map(|s| s.session) {
                println!(
                    "ChatGPT: connected; account={} plan_usage={} expiry={}",
                    session.email.as_deref().unwrap_or("(no email)"),
                    session.scopes.iter().any(|s| s == PERMISSION),
                    session.expires_at
                );
            } else {
                println!("ChatGPT: not signed in");
            }
            Ok(())
        }
        AccountAction::Models => {
            let body = model_list().await?;
            let items = body
                .get("models")
                .and_then(Value::as_array)
                .context("ChatGPT account model catalog unavailable")?;
            for entry in items {
                if entry.get("visibility").and_then(Value::as_str) == Some("list") {
                    let id = entry.get("slug").and_then(Value::as_str).unwrap_or("");
                    let name = entry
                        .get("display_name")
                        .and_then(Value::as_str)
                        .unwrap_or(id);
                    println!("{id}\t{name}");
                }
            }
            Ok(())
        }
        AccountAction::Logout => {
            let mut stored = read_saved()?.context("Not signed in")?;
            let revocation = async {
                if let Some(session) = &stored.session {
                    let http = new_client()?;
                    let config: Value = http
                        .get(format!("{AUTH}/.well-known/openid-configuration"))
                        .send()
                        .await?
                        .error_for_status()?
                        .json()
                        .await?;
                    let endpoint = config
                        .get("revocation_endpoint")
                        .and_then(Value::as_str)
                        .context("OAuth discovery lacks revocation endpoint")?;
                    let endpoint = Url::parse(endpoint)?;
                    if endpoint.scheme() != "https"
                        || endpoint.host_str() != Some("auth.openai.com")
                    {
                        bail!("Invalid OpenAI revocation endpoint")
                    }
                    http.post(endpoint)
                        .form(&[
                            ("token", session.refresh_token.as_str()),
                            ("token_type_hint", "refresh_token"),
                            ("client_id", session.client_id.as_str()),
                        ])
                        .send()
                        .await?
                        .error_for_status()?;
                }
                Ok::<(), anyhow::Error>(())
            }
            .await;
            stored.session = None;
            write_saved(&stored)?;
            match revocation {
                Ok(()) => println!("ChatGPT session revoked; local credentials cleared."),
                Err(error) => {
                    eprintln!("Local ChatGPT credentials cleared; remote revocation was not confirmed: {error:#}");
                    eprintln!("Disconnect the application from ChatGPT Settings if necessary.");
                }
            }
            Ok(())
        }
    }
}

fn input_items(request: &ModelRequest) -> Result<Vec<Value>> {
    let mut items =
        vec![json!({"role":"developer","content":[{"type":"input_text","text":request.system}]})];
    for message in &request.messages {
        match message.role.as_str() {
            "tool" => {
                items.push(json!({
                    "type":"function_call_output",
                    "call_id":message.tool_call_id.as_deref().context("Missing call ID in tool output")?,
                    "output":message.visible_text(),
                }));
            }
            "user" | "assistant" => {
                if message.role == "assistant" {
                    if let Some(encoded) = message
                        .reasoning_content
                        .as_deref()
                        .and_then(|value| value.strip_prefix(REASONING_ITEMS_PREFIX))
                    {
                        let reasoning: Vec<Value> = serde_json::from_str(encoded)
                            .context("Invalid stored Responses reasoning items")?;
                        for item in reasoning {
                            if item.get("type").and_then(Value::as_str) != Some("reasoning")
                                || item.get("encrypted_content").and_then(Value::as_str).is_none()
                            {
                                bail!("Stored Responses reasoning item is missing encrypted content");
                            }
                            items.push(item);
                        }
                    }
                }
                let mut content = Vec::new();
                if let Some(blocks) = &message.content_blocks {
                    for block in crate::multimodal::openai(blocks)? {
                        match block.get("type").and_then(Value::as_str) {
                            Some("text") => content.push(json!({
                                "type": if message.role == "assistant" {"output_text"} else {"input_text"},
                                "text":block.get("text").and_then(Value::as_str).unwrap_or(""),
                            })),
                            Some("image_url") if message.role == "user" => content.push(json!({
                                "type":"input_image",
                                "image_url":block.pointer("/image_url/url").and_then(Value::as_str)
                                    .context("Missing image URI")?,
                            })),
                            _ => bail!("Unsupported ChatGPT Responses input block"),
                        }
                    }
                } else if let Some(text) = message.content.as_deref().filter(|text| !text.is_empty()) {
                    content.push(json!({
                        "type": if message.role == "assistant" {"output_text"} else {"input_text"},
                        "text":text,
                    }));
                }
                if !content.is_empty() {
                    items.push(json!({"role":message.role,"content":content}));
                }
                if let Some(calls) = &message.tool_calls {
                    for call in calls {
                        items.push(json!({
                            "type":"function_call",
                            "call_id":call.id,
                            "name":format!("atlas.{}", call.function.name),
                            "arguments":call.function.arguments,
                        }));
                    }
                }
            }
            role => bail!("Unsupported ChatGPT Responses role: {role}"),
        }
    }
    Ok(items)
}

fn response_tools(tools: &[Value]) -> Result<Value> {
    let functions = tools.iter().map(|tool| {
        let func = tool.get("function").context("Unsupported tool spec")?;
        let name = func.get("name").and_then(Value::as_str).context("Tool has no name")?;
        Ok(json!({
            "type":"function",
            "name":name,
            "description":func.get("description").and_then(Value::as_str).unwrap_or(""),
            "parameters":func.get("parameters").cloned().unwrap_or_else(|| json!({"type":"object","properties":{}})),
            "strict":false,
        }))
    }).collect::<Result<Vec<_>>>()?;
    Ok(json!([{
        "type":"namespace",
        "name":"atlas",
        "description":"Local coding agent tools, run in the user's workspace",
        "tools":functions,
    }]))
}

fn parse_completed(response: &Value) -> Result<ModelResponse> {
    let mut content = String::new();
    let mut calls = Vec::new();
    let mut reasoning = Vec::new();
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .context("ChatGPT Responses completion omitted output array")?;
    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("reasoning") => {
                if item.get("encrypted_content").and_then(Value::as_str).is_some() {
                    reasoning.push(item.clone());
                }
            }
            Some("function_call") => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .context("Function call missing call_id")?;
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .context("Function call missing name")?;
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .context("Function call missing arguments")?;
                calls.push(ToolCall {
                    id: call_id.into(),
                    kind: "function".into(),
                    function: FunctionCall {
                        name: name.strip_prefix("atlas.").unwrap_or(name).into(),
                        arguments: arguments.into(),
                    },
                });
            }
            Some("message") => {
                if let Some(blocks) = item.get("content").and_then(Value::as_array) {
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) == Some("output_text") {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                content.push_str(text);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let usage = response.get("usage").map(|value| {
        let input = value
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let output = value
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input.saturating_add(output),
            cache_read_input_tokens: value
                .pointer("/input_tokens_details/cached_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ..Default::default()
        }
    });
    let has_calls = !calls.is_empty();
    Ok(ModelResponse {
        message: Message {
            role: "assistant".into(),
            content: Some(content),
            content_blocks: None,
            tool_calls: has_calls.then_some(calls),
            tool_call_id: None,
            reasoning_content: if reasoning.is_empty() {
                None
            } else {
                Some(format!(
                    "{REASONING_ITEMS_PREFIX}{}",
                    serde_json::to_string(&reasoning)?
                ))
            },
        },
        usage,
        finish_reason: Some(if has_calls { "tool_calls" } else { "stop" }.into()),
    })
}

fn next_sse_frame(buffer: &[u8]) -> Option<(usize, usize)> {
    for (i, byte) in buffer.iter().enumerate() {
        if *byte == b'\n' {
            if buffer.get(i + 1) == Some(&b'\n') {
                return Some((i, 2));
            }
            if buffer.get(i + 1) == Some(&b'\r') && buffer.get(i + 2) == Some(&b'\n') {
                return Some((i, 3));
            }
        }
    }
    None
}

pub async fn complete(
    request: &ModelRequest,
    tuning: &ModelTuning,
    deltas: Option<UnboundedSender<String>>,
) -> Result<ModelResponse> {
    let token = active_token().await?;
    complete_with_token(request, tuning, deltas, &token, true, None, RESOURCE).await
}

/// Ordinary OpenAI API keys use the same stateless Responses adapter, with
/// the public API's model settings rather than subscription restrictions.
pub async fn complete_api(
    request: &ModelRequest,
    tuning: &ModelTuning,
    deltas: Option<UnboundedSender<String>>,
    api_key: &str,
    response_schema: Option<&Value>,
    base_url: &str,
) -> Result<ModelResponse> {
    crate::config::require_secure_credential_transport(base_url, true)?;
    complete_with_token(
        request,
        tuning,
        deltas,
        api_key,
        false,
        response_schema,
        base_url,
    )
    .await
}

async fn complete_with_token(
    request: &ModelRequest,
    tuning: &ModelTuning,
    deltas: Option<UnboundedSender<String>>,
    token: &str,
    subscription: bool,
    response_schema: Option<&Value>,
    base_url: &str,
) -> Result<ModelResponse> {
    let mut body = json!({
        "model":request.model,
        "store":false,
        "stream":true,
        "input":input_items(request)?,
    });
    if !request.tools.is_empty() {
        body["tools"] = json!(response_tools(&request.tools)?);
    }
    if let Some(schema) = response_schema {
        body["text"]["format"] = json!({
            "type":"json_schema",
            "name":"structured_output",
            "strict":true,
            "schema":schema
        });
    }
    if subscription {
        if tuning.max_output_tokens.is_some()
            || tuning.temperature.is_some()
            || tuning.top_p.is_some()
        {
            bail!("ChatGPT plan disallows max_output_tokens, temperature and top_p")
        }
    } else {
        if let Some(max) = tuning.max_output_tokens {
            body["max_output_tokens"] = json!(max);
        }
        if tuning.temperature.is_some() || tuning.top_p.is_some() {
            if tuning.reasoning_effort.as_deref() != Some("none") {
                bail!(
                    "OpenAI reasoning models require reasoning_effort=none before custom sampling"
                );
            }
            if let Some(value) = tuning.temperature {
                body["temperature"] = json!(value);
            }
            if let Some(value) = tuning.top_p {
                body["top_p"] = json!(value);
            }
        }
    }
    if let Some(effort) = &tuning.reasoning_effort {
        body["reasoning"] = json!({"effort":effort});
    }
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        inference_client()?
            .post(format!("{}/responses", base_url.trim_end_matches('/')))
            .bearer_auth(token)
            .json(&body)
            .send(),
    )
    .await
    .context("OpenAI Responses headers timed out")??
    .error_for_status()
    .context("OpenAI Responses inference rejected")?;
    let mut stream = response.bytes_stream();
    let mut buffered = Vec::new();
    let mut completed = None;
    loop {
        let Some(chunk) =
            tokio::time::timeout(crate::provider::stream_idle_timeout(), stream.next())
                .await
                .context("OpenAI Responses stream idle timeout")?
        else {
            break;
        };
        let chunk = chunk.context("OpenAI Responses stream interrupted")?;
        buffered.extend_from_slice(&chunk);
        if buffered.len() > 16 * 1024 * 1024 {
            bail!("Responses SSE frame exceeds limit")
        }
        while let Some((end, delimiter)) = next_sse_frame(&buffered) {
            let frame = buffered.drain(..end + delimiter).collect::<Vec<_>>();
            let data = String::from_utf8_lossy(&frame);
            let lines = data
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>();
            if lines.is_empty() {
                continue;
            }
            let payload = lines.join("\n");
            if payload.trim() == "[DONE]" {
                continue;
            }
            let event: Value =
                serde_json::from_str(&payload).context("Invalid OpenAI Responses event")?;
            match event.get("type").and_then(Value::as_str) {
                Some("response.output_text.delta") => {
                    if let (Some(tx), Some(text)) =
                        (&deltas, event.get("delta").and_then(Value::as_str))
                    {
                        let _ = tx.send(text.to_owned());
                    }
                }
                Some("response.completed") => {
                    completed = Some(
                        event
                            .get("response")
                            .context("Missing final Responses body")?
                            .clone(),
                    );
                }
                Some("response.failed" | "response.incomplete" | "error") => {
                    let code = event
                        .pointer("/response/error/code")
                        .or_else(|| event.pointer("/error/code"))
                        .or_else(|| event.get("code"))
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    bail!(
                        "ChatGPT Responses operation failed: {} ({code})",
                        event
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                    )
                }
                _ => {}
            }
        }
    }
    parse_completed(
        &completed.context("ChatGPT Responses stream ended without response.completed")?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translates_tools_and_history_to_responses_items() {
        let call = ToolCall {
            id: "call1".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        let request = ModelRequest {
            model: "test".into(),
            system: "rules".into(),
            messages: vec![
                Message::text("user", "hello"),
                Message {
                    role: "assistant".into(),
                    content: None,
                    content_blocks: None,
                    tool_calls: Some(vec![call.clone()]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                Message::tool_result(&call, "done".into()),
            ],
            tools: vec![json!({"type":"function","function":{"name":"read_file",
                "description":"read file","parameters":{"type":"object","properties":{}}}})],
        };
        let wire = input_items(&request).unwrap();
        assert_eq!(wire[0]["role"], "developer");
        assert_eq!(wire[2]["type"], "function_call");
        assert_eq!(wire[3]["type"], "function_call_output");
        assert_eq!(
            response_tools(&request.tools).unwrap()[0]["type"],
            "namespace"
        );
        assert_eq!(
            response_tools(&request.tools).unwrap()[0]["tools"][0]["name"],
            "read_file"
        );
    }
    #[test]
    fn tool_calls_and_usage_are_kept_at_response_completion() {
        let parsed = parse_completed(&json!({
            "output":[{"type":"function_call","call_id":"a","name":"Bash","arguments":"{}"},
                {"type":"message","content":[{"type":"output_text","text":"hello"}]}],
            "usage":{"input_tokens":10,"output_tokens":5,
                "input_tokens_details":{"cached_tokens":2}}
        }))
        .unwrap();
        assert_eq!(parsed.message.content.as_deref(), Some("hello"));
        assert_eq!(parsed.message.tool_calls.unwrap()[0].id, "a");
        assert_eq!(parsed.usage.unwrap().cache_read_input_tokens, 2);
    }

    #[tokio::test]
    async fn api_key_responses_stream_preserves_tools_schema_and_usage() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4096];
            let end = loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
            assert!(headers.starts_with("post /v1/responses http/1.1"));
            assert!(headers.contains("authorization: bearer local-openai-key"));
            let length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while bytes.len() < end + length {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let body: Value = serde_json::from_slice(&bytes[end..end + length]).unwrap();
            assert_eq!(body["store"], false);
            assert_eq!(body["stream"], true);
            assert_eq!(body["model"], "gpt-6.1-sol");
            assert_eq!(body["tools"][0]["type"], "namespace");
            assert_eq!(body["tools"][0]["tools"][0]["name"], "read_file");
            assert_eq!(body["text"]["format"]["type"], "json_schema");
            let response = json!({"type":"response.completed","response":{
                "output":[{"type":"message","content":[{"type":"output_text","text":"Hello"}]},
                    {"type":"function_call","name":"atlas.read_file","call_id":"call-a","arguments":"{}"}],
                "usage":{"input_tokens":12,"output_tokens":4}
            }});
            let payload = format!(
                "data: {}\r\n\r\ndata: {}\r\n\r\ndata: [DONE]\r\n\r\n",
                json!({"type":"response.output_text.delta","delta":"Hello"}),
                response
            );
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len());
            socket.write_all(head.as_bytes()).await.unwrap();
            for part in payload.as_bytes().chunks(19) {
                socket.write_all(part).await.unwrap();
            }
        });
        let req = ModelRequest {
            model: "gpt-6.1-sol".into(),
            system: "local project".into(),
            messages: vec![Message::text("user", "read source")],
            tools: vec![json!({"type":"function","function":{"name":"read_file",
                "description":"Read","parameters":{"type":"object","properties":{}}}})],
        };
        let schema = json!({"type":"object","properties":{"result":{"type":"string"}}});
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let reply = complete_api(
            &req,
            &ModelTuning::default(),
            Some(tx),
            "local-openai-key",
            Some(&schema),
            &format!("http://{addr}/v1"),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), "Hello");
        assert_eq!(reply.message.content.as_deref(), Some("Hello"));
        assert_eq!(
            reply.message.tool_calls.as_ref().unwrap()[0].function.name,
            "read_file"
        );
        assert_eq!(reply.usage.unwrap().total_tokens, 16);
    }

    #[test]
    fn stateless_reasoning_items_survive_tool_roundtrips() {
        let item = json!({
            "type": "reasoning",
            "id": "rs_opaque",
            "summary": [],
            "encrypted_content": "opaque-ciphertext"
        });
        let parsed = parse_completed(&json!({
            "output": [
                item,
                {
                    "type": "function_call",
                    "call_id": "call-one",
                    "name": "atlas.read_file",
                    "arguments": "{}"
                }
            ]
        }))
        .unwrap();
        assert!(parsed
            .message
            .reasoning_content
            .as_deref()
            .unwrap()
            .starts_with(REASONING_ITEMS_PREFIX));
        let request = ModelRequest {
            model: "gpt-6.1-sol".into(),
            system: "rules".into(),
            messages: vec![
                Message::text("user", "read source"),
                parsed.message.clone(),
                Message::tool_result(
                    &parsed.message.tool_calls.as_ref().unwrap()[0],
                    "file contents".into(),
                ),
            ],
            tools: vec![],
        };
        let wire = input_items(&request).unwrap();
        assert_eq!(wire[2]["type"], "reasoning");
        assert_eq!(wire[2]["encrypted_content"], "opaque-ciphertext");
        assert_eq!(wire[3]["type"], "function_call");
        assert_eq!(wire[4]["type"], "function_call_output");
    }

    #[test]
    fn chatgpt_subscription_permission_is_checked_exactly() {
        assert!(scope_granted(
            "openid chatgpt.tokens.use.direct offline_access"
        ));
        assert!(!scope_granted("openid chatgpt.tokens.use.directX"));
    }
}
