use crate::config::SearchBackend;
use anyhow::{bail, Context, Result};
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::time::Duration;

pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"web_search",
        "description":"Search the web through the user's explicitly configured SearXNG or Brave Search service. Requires external network approval.",
        "parameters":{"type":"object","properties":{
            "query":{"type":"string"},
            "limit":{"type":"integer","minimum":1,"maximum":10}
        },"required":["query"]}
    }})
}

pub async fn execute(backend: &SearchBackend, args: &Value) -> Result<String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .context("Search query missing")?;
    if query.trim().is_empty() {
        bail!("Search query must not be empty")
    }
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(8)
        .clamp(1, 10);
    let client = Client::builder().timeout(Duration::from_secs(20)).build()?;
    let endpoint = Url::parse(&backend.base_url)?;
    if !matches!(endpoint.scheme(), "http" | "https") {
        bail!("Search endpoint must be http(s)")
    }
    let response = match backend.kind.as_str() {
        "searxng" => {
            let mut url = endpoint.join("search")?;
            url.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("format", "json");
            client.get(url).send().await?
        }
        "brave" => {
            let env = backend
                .api_key_env
                .as_deref()
                .context("Brave search requires api_key_env")?;
            let key = std::env::var(env)
                .with_context(|| format!("Missing search API credential in {env}"))?;
            let mut url = endpoint.join("res/v1/web/search")?;
            url.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("count", &limit.to_string());
            client
                .get(url)
                .header("X-Subscription-Token", key)
                .send()
                .await?
        }
        other => bail!("Unsupported search backend {other}"),
    };
    let status = response.status();
    let value: Value = response.json().await?;
    if !status.is_success() {
        bail!("Search endpoint HTTP {status}")
    }
    let rows = match backend.kind.as_str() {
        "searxng" => value.get("results").and_then(Value::as_array),
        "brave" => value.pointer("/web/results").and_then(Value::as_array),
        _ => None,
    }
    .context("Search endpoint returned no result array")?;
    let results = rows
        .iter()
        .take(limit as usize)
        .map(|result| {
            json!({
                "title":result.get("title").and_then(Value::as_str).unwrap_or(""),
                "url":result.get("url").and_then(Value::as_str).unwrap_or(""),
                "description":result.get(if backend.kind=="brave"{"description"}else{"content"})
                    .and_then(Value::as_str).unwrap_or(""),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"query":query,"results":results}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn searxng_local_mock_results_normalized() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0u8; 4096];
            let n = socket.read(&mut bytes).await.unwrap();
            let request = String::from_utf8_lossy(&bytes[..n]);
            assert!(request.contains("format=json"));
            assert!(request.contains("q=Rust"));
            let body = json!({"results":[
                {"title":"First","url":"https://example.org/a","content":"A"},
                {"title":"Second","url":"https://example.org/b","content":"B"}
            ]})
            .to_string();
            let packet=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
            socket.write_all(packet.as_bytes()).await.unwrap();
        });
        let config = SearchBackend {
            kind: "searxng".into(),
            base_url: format!("http://{address}/"),
            api_key_env: None,
        };
        let raw = execute(&config, &json!({"query":"Rust","limit":1}))
            .await
            .unwrap();
        server.await.unwrap();
        let result: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(result["results"].as_array().unwrap().len(), 1);
        assert_eq!(result["results"][0]["title"], "First");
    }
}
