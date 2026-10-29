use anyhow::{bail, Context, Result};
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

const MAX_BODY: usize = 1_000_000;

fn acceptable_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let [a, b, c, _] = v.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 0))
        }
        IpAddr::V6(v) => {
            !(v.is_loopback()
                || v.is_unspecified()
                || v.is_unique_local()
                || v.is_unicast_link_local()
                || v.to_ipv4_mapped()
                    .is_some_and(|ip| !acceptable_ip(IpAddr::V4(ip))))
        }
    }
}

async fn verify_public(url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https") {
        bail!("Only http(s) URLs supported")
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("URL credentials cannot be embedded")
    }
    let host = url.host_str().context("URL missing host")?;
    if matches!(host, "localhost" | "localhost.localdomain")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
    {
        bail!("Local addresses are not allowed in web_fetch")
    }
    let port = url.port_or_known_default().context("URL missing port")?;
    let addresses = tokio::net::lookup_host((host, port)).await?;
    let mut found = false;
    for address in addresses {
        found = true;
        if !acceptable_ip(address.ip()) {
            bail!("Non-public destination blocked: {}", address.ip());
        }
    }
    if !found {
        bail!("Host has no resolved addresses")
    }
    Ok(())
}
fn sanitize_html(body: &str) -> String {
    let mut text = body.to_owned();
    for (pattern, replacement) in [
        (
            r"(?is)<(script|style|noscript)\b[^>]*>.*?</(script|style|noscript)\s*>",
            "",
        ),
        (r"(?i)</(p|div|h[1-6]|li|ul|ol|pre|tr)\s*>", "\n"),
        (r"(?i)<br\s*/?>", "\n"),
        (r"(?is)<[^>]*>", ""),
    ] {
        let regex = Regex::new(pattern).unwrap();
        text = regex.replace_all(&text, replacement).to_string();
    }
    for (entity, decoded) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        text = text.replace(entity, decoded);
    }
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn fetch_with_redirect_policy(
    url: &str,
    allow_private_for_test: bool,
    approve_redirect: &(impl Fn(&Url) -> Result<()> + Sync),
) -> Result<Value> {
    let client = Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("AtlasAgent/0.1 (+user-requested-web-fetch)")
        .build()?;
    let mut current = Url::parse(url)?;
    for _ in 0..5 {
        if !allow_private_for_test {
            verify_public(&current).await?;
        }
        let mut response = client
            .get(current.clone())
            .send()
            .await
            .with_context(|| format!("GET {current}"))?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get("location")
                .context("HTTP redirect missing Location")?
                .to_str()
                .context("Invalid redirect Location")?;
            let next = current.join(location)?;
            // Tool approval is scoped to the original hostname. Following a
            // redirect to another host requires a fresh policy decision
            // *before* the next HTTP request is sent.
            if next.host_str() != current.host_str() {
                approve_redirect(&next)?;
            }
            current = next;
            continue;
        }
        let code = response.status();
        if !code.is_success() {
            bail!("HTTP {code} at {current}")
        }
        let kind = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        if !(kind.is_empty()
            || kind.contains("text")
            || kind.contains("json")
            || kind.contains("xml")
            || kind.contains("html"))
        {
            bail!("web_fetch only accepts textual content")
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BODY as u64)
        {
            bail!("Web response larger than {MAX_BODY} bytes")
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_BODY {
                bail!("Web response exceeds size limit")
            }
            bytes.extend_from_slice(&chunk);
        }
        let body = String::from_utf8_lossy(&bytes);
        let text = if kind.contains("html") {
            sanitize_html(&body)
        } else {
            body.to_string()
        };
        return Ok(json!({
            "url":current.as_str(),"code":code.as_u16(),"bytes":bytes.len(),
            "content":text.chars().take(40_000).collect::<String>()
        }));
    }
    bail!("Web redirect limit exceeded")
}

async fn fetch(url: &str, allow_private_for_test: bool) -> Result<Value> {
    fetch_with_redirect_policy(url, allow_private_for_test, &|_| Ok(())).await
}

pub async fn execute_with_redirect_policy(
    args: &Value,
    approve_redirect: &(impl Fn(&Url) -> Result<()> + Sync),
) -> Result<String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .context("web_fetch requires url")?;
    let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or("");
    let mut output = fetch_with_redirect_policy(url, false, approve_redirect).await?;
    output["prompt"] = json!(prompt);
    Ok(output.to_string())
}

pub async fn execute(args: &Value) -> Result<String> {
    execute_with_redirect_policy(args, &|_| Ok(())).await
}

pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"web_fetch",
        "description":"Fetch public web text (HTML converted to plain text). This only runs after explicit network approval. Model applies the prompt to fetched content.",
        "parameters":{"type":"object","properties":{
            "url":{"type":"string"},"prompt":{"type":"string"}
        },"required":["url","prompt"]}
    }})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_non_public_ip_ranges() {
        for host in [
            "127.0.0.1",
            "10.0.0.10",
            "192.168.1.3",
            "172.20.2.1",
            "169.254.1.2",
            "100.64.1.1",
            "::1",
        ] {
            assert!(!acceptable_ip(host.parse().unwrap()), "{host}");
        }
        assert!(acceptable_ip("8.8.8.8".parse().unwrap()));
    }
    #[test]
    fn html_sanitization_removes_scripts() {
        let html = "<h1>Heading</h1><script>alert('secret')</script><p>One &amp; Two</p>";
        let result = sanitize_html(html);
        assert!(result.contains("Heading"));
        assert!(result.contains("One & Two"));
        assert!(!result.contains("alert"));
    }
    #[tokio::test]
    async fn cross_hostname_redirect_cannot_bypass_explicit_deny_rule() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            stream.read(&mut buf).await.unwrap();
            let location = "http://localhost:9/do-not-fetch";
            let response=format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let rules =
            crate::permission_rules::compile_rules(&[crate::permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "WebFetch(domain:localhost)".into(),
            }])
            .unwrap();
        let url = format!("http://{address}/start");
        let denied = fetch_with_redirect_policy(&url, true, &|next| {
            match crate::permission_rules::policy_for(
                &rules,
                "web_fetch",
                &json!({"url":next.as_str()}),
            )? {
                Some(crate::permission_rules::Behavior::Deny) => {
                    bail!("redirect destination explicitly denied")
                }
                _ => Ok(()),
            }
        })
        .await
        .unwrap_err();
        assert!(denied.to_string().contains("explicitly denied"));
        server.await.unwrap();
    }
    #[tokio::test]
    async fn web_fetch_reads_local_mock_only_with_internal_test_override() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            stream.read(&mut buffer).await.unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 18\r\nConnection: close\r\n\r\n<h1>Web hello</h1>").await.unwrap();
        });
        let url = format!("http://{address}/test");
        assert!(verify_public(&Url::parse(&url).unwrap()).await.is_err());
        let result = fetch(&url, true).await.unwrap();
        server.await.unwrap();
        assert_eq!(result["code"], 200);
        assert_eq!(result["content"], "Web hello");
    }
}
