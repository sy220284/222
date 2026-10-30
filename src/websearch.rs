use crate::config::SearchBackend;
use anyhow::{bail, Context, Result};
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"web_search",
        "description":"Search the web through the user's explicitly configured SearXNG or Brave Search service. Requires external network approval.",
        "parameters":{"type":"object","properties":{
            "query":{"type":"string"},
            "limit":{"type":"integer","minimum":1,"maximum":10},
            "allowed_domains":{"type":"array","items":{"type":"string"}},
            "blocked_domains":{"type":"array","items":{"type":"string"}}
        },"required":["query"]}
    }})
}

fn domain_filter(args: &Value, key: &str) -> Result<Vec<String>> {
    let Some(raw) = args.get(key) else {
        return Ok(Vec::new());
    };
    let domains = raw
        .as_array()
        .with_context(|| format!("{key} must be an array"))?;
    domains
        .iter()
        .map(|item| {
            let domain = item
                .as_str()
                .with_context(|| format!("{key} entries must be strings"))?
                .trim()
                .trim_end_matches('.')
                .to_ascii_lowercase();
            if domain.is_empty() || domain.chars().any(|c| matches!(c, '/' | ':' | '@' | ' ')) {
                bail!("{key} must contain hostnames without schemes or paths");
            }
            Ok(domain)
        })
        .collect()
}

fn domain_allowed(url: &str, allowed: &[String], blocked: &[String]) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let matches_domain = |domain: &String| {
        host.as_str() == domain.as_str() || host.ends_with(format!(".{domain}").as_str())
    };
    !blocked.iter().any(|domain| matches_domain(domain))
        && (allowed.is_empty() || allowed.iter().any(|domain| matches_domain(domain)))
}

pub async fn execute(backend: &SearchBackend, args: &Value) -> Result<String> {
    let started = Instant::now();
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .context("Search query missing")?;
    if query.trim().encode_utf16().count() < 2 {
        bail!("Search query must contain at least two UTF-16 code units")
    }
    let allowed = domain_filter(args, "allowed_domains")?;
    let blocked = domain_filter(args, "blocked_domains")?;
    // WebSearchTool.ts::validateInput rejects mutually exclusive domain lists
    // before dispatching any network request.
    if !allowed.is_empty() && !blocked.is_empty() {
        bail!("Error: Cannot specify both allowed_domains and blocked_domains in the same request");
    }
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(8)
        .clamp(1, 10);
    crate::config::require_secure_credential_transport(
        &backend.base_url,
        backend.kind == "brave" && backend.api_key_env.is_some(),
    )?;
    let client = crate::tls_ca::client_builder()?
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
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
    if !status.is_success() {
        bail!("Search endpoint HTTP {status}")
    }
    let value: Value = response.json().await?;
    let rows = match backend.kind.as_str() {
        "searxng" => value.get("results").and_then(Value::as_array),
        "brave" => value.pointer("/web/results").and_then(Value::as_array),
        _ => None,
    }
    .context("Search endpoint returned no result array")?;
    let results = rows
        .iter()
        .filter(|result| {
            result
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|url| domain_allowed(url, &allowed, &blocked))
        })
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
    Ok(
        json!({"query":query,"results":results,"durationSeconds":started.elapsed().as_secs_f64()})
            .to_string(),
    )
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
