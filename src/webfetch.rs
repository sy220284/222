use anyhow::{bail, Context, Result};
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use uuid::Uuid;

// Source WebFetchTool/utils.ts MAX_HTTP_CONTENT_LENGTH = 10 * 1024 * 1024.
const MAX_BODY: usize = 10 * 1024 * 1024;
const MAX_MARKDOWN_UTF16: usize = 100_000;
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const CACHE_CAP_BYTES: usize = 50 * 1024 * 1024;

struct CachedPage {
    key: String,
    value: Value,
    size: usize,
    fetched: Instant,
}

static URL_CACHE: OnceLock<Mutex<VecDeque<CachedPage>>> = OnceLock::new();

fn cache() -> &'static Mutex<VecDeque<CachedPage>> {
    URL_CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn cached_page(key: &str) -> Option<Value> {
    let mut entries = cache().lock().unwrap();
    entries.retain(|entry| entry.fetched.elapsed() < CACHE_TTL);
    let position = entries.iter().position(|entry| entry.key == key)?;
    let entry = entries.remove(position)?;
    let value = entry.value.clone();
    // An evicted session attachment must be fetched and saved again.
    if value
        .get("persistedPath")
        .and_then(Value::as_str)
        .is_some_and(|path| !Path::new(path).is_file())
    {
        return None;
    }
    entries.push_back(entry);
    Some(value)
}

fn cache_page(key: String, value: Value, size: usize) {
    if size > CACHE_CAP_BYTES {
        return;
    }
    let mut entries = cache().lock().unwrap();
    entries.retain(|entry| entry.key != key && entry.fetched.elapsed() < CACHE_TTL);
    let mut total: usize = entries.iter().map(|entry| entry.size).sum();
    while total.saturating_add(size) > CACHE_CAP_BYTES {
        let Some(evicted) = entries.pop_front() else {
            break;
        };
        total = total.saturating_sub(evicted.size);
    }
    entries.push_back(CachedPage {
        key,
        value,
        size: size.max(1),
        fetched: Instant::now(),
    });
}

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
            let first = v.segments()[0];
            !(v.is_loopback()
                || v.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || v.to_ipv4_mapped()
                    .is_some_and(|ip| !acceptable_ip(IpAddr::V4(ip))))
        }
    }
}

async fn verify_public(url: &Url) -> Result<Vec<std::net::SocketAddr>> {
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
    let addresses: Vec<_> = tokio::net::lookup_host((host, port)).await?.collect();
    if addresses.is_empty() {
        bail!("Host has no resolved addresses")
    }
    for address in &addresses {
        if !acceptable_ip(address.ip()) {
            bail!("Non-public destination blocked: {}", address.ip());
        }
    }
    // The request must connect to the address that passed validation.
    // Resolving again inside the HTTP client permits DNS rebinding.
    Ok(addresses)
}

fn permitted_redirect(from: &Url, to: &Url) -> bool {
    fn strip_www(host: &str) -> &str {
        host.strip_prefix("www.").unwrap_or(host)
    }
    from.scheme() == to.scheme()
        && from.port_or_known_default() == to.port_or_known_default()
        && to.username().is_empty()
        && to.password().is_none()
        && from
            .host_str()
            .zip(to.host_str())
            .is_some_and(|(a, b)| strip_www(a) == strip_www(b))
}

fn html_attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?i)\b{}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#,
        regex::escape(name)
    );
    let pattern = Regex::new(&pattern).ok()?;
    let captures = pattern.captures(tag)?;
    let value = (1..=3).find_map(|i| captures.get(i).map(|m| m.as_str()))?;
    Some(decode_html_entities(value))
}

fn safe_html_link(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:")
        || lower.starts_with('/')
        || lower.starts_with('#')
        || (!lower.contains(':') && !lower.starts_with("//"))
}

fn decode_html_entities(text: &str) -> String {
    let entities = Regex::new(r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);").unwrap();
    entities
        .replace_all(text, |caps: &regex::Captures<'_>| {
            let entity = &caps[1];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "hellip" => Some('…'),
                _ if entity.starts_with("#x") => u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32),
                _ if entity.starts_with('#') => {
                    entity[1..].parse::<u32>().ok().and_then(char::from_u32)
                }
                _ => None,
            };
            decoded
                .map(|ch| ch.to_string())
                .unwrap_or_else(|| caps[0].to_owned())
        })
        .into_owned()
}

/// Convert common HTML document structures to Markdown for model consumption.
/// The original uses Turndown; malformed/nested DOM edge cases remain open.
fn sanitize_html(body: &str) -> String {
    let invisible = Regex::new(
        r"(?is)<(script|style|noscript|template)\b[^>]*>.*?</(script|style|noscript|template)\s*>",
    )
    .unwrap();
    let cleaned = invisible.replace_all(body, "");
    let tokens = Regex::new(r"(?s)<!--.*?-->|<[^>]+>|[^<]+").unwrap();
    let mut output = String::new();
    let mut lists: Vec<(bool, usize)> = Vec::new();
    let mut links: Vec<Option<String>> = Vec::new();
    let mut preformatted = false;
    for token in tokens.find_iter(&cleaned) {
        let token = token.as_str();
        if token.starts_with("<!--") {
            continue;
        }
        if !token.starts_with('<') {
            let decoded = decode_html_entities(token);
            if preformatted {
                output.push_str(&decoded);
            } else {
                let collapsed = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
                if !collapsed.is_empty() {
                    if decoded.chars().next().is_some_and(char::is_whitespace)
                        && !output.chars().last().is_some_and(char::is_whitespace)
                    {
                        output.push(' ');
                    }
                    output.push_str(&collapsed);
                    if decoded.chars().last().is_some_and(char::is_whitespace) {
                        output.push(' ');
                    }
                }
            }
            continue;
        }
        let closing = token.starts_with("</");
        let raw = token.trim_start_matches('<').trim_start_matches('/').trim();
        let tag = raw
            .split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let heading = if tag.len() == 2 && tag.starts_with('h') {
            tag.as_bytes()[1]
                .checked_sub(b'0')
                .filter(|n| (1..=6).contains(n))
        } else {
            None
        };
        if let Some(level) = heading {
            if !closing {
                output.push_str("\n\n");
                output.push_str(&"#".repeat(level as usize));
                output.push(' ');
            } else {
                output.push_str("\n\n");
            }
            continue;
        }
        match (closing, tag.as_str()) {
            (false, "pre") => {
                preformatted = true;
                output.push_str("\n\n");
                output.push_str(&char::from(96).to_string().repeat(3));
                output.push('\n');
            }
            (true, "pre") => {
                preformatted = false;
                output.push('\n');
                output.push_str(&char::from(96).to_string().repeat(3));
                output.push_str("\n\n");
            }
            (_, "code") if !preformatted => output.push(char::from(96)),
            (_, "strong" | "b") => output.push_str("**"),
            (_, "em" | "i") => output.push('*'),
            (_, "del" | "s" | "strike") => output.push_str("~~"),
            (false, "ul") => {
                lists.push((false, 0));
                output.push('\n');
            }
            (false, "ol") => {
                lists.push((true, 0));
                output.push('\n');
            }
            (true, "ul" | "ol") => {
                lists.pop();
                output.push('\n');
            }
            (false, "li") => {
                output.push('\n');
                output.push_str(&"  ".repeat(lists.len().saturating_sub(1)));
                if let Some((ordered, count)) = lists.last_mut() {
                    if *ordered {
                        *count += 1;
                        output.push_str(&format!("{}. ", count));
                    } else {
                        output.push_str("- ");
                    }
                } else {
                    output.push_str("- ");
                }
            }
            (true, "li") => output.push('\n'),
            (false, "a") => {
                let href = html_attribute(token, "href").filter(|url| safe_html_link(url));
                if href.is_some() {
                    output.push('[');
                }
                links.push(href);
            }
            (true, "a") => {
                if let Some(Some(href)) = links.pop() {
                    output.push_str(&format!("]({href})"));
                }
            }
            (false, "img") => {
                let src = html_attribute(token, "src").unwrap_or_default();
                if !src.is_empty() && safe_html_link(&src) {
                    let alt = html_attribute(token, "alt").unwrap_or_default();
                    output.push_str(&format!("![{alt}]({src})"));
                }
            }
            (false, "br") => output.push('\n'),
            (false, "hr") => output.push_str("\n\n---\n\n"),
            (false, "blockquote") => output.push_str("\n\n> "),
            (true, "blockquote") => output.push_str("\n\n"),
            (false, "tr") => output.push('\n'),
            (true, "td" | "th") => output.push_str(" | "),
            (_, "p" | "div" | "section" | "article" | "header" | "footer" | "table") => {
                output.push_str("\n\n");
            }
            _ => {}
        }
    }
    let mut lines = Vec::new();
    let mut blank = false;
    for line in output.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if !blank {
                lines.push("");
            }
            blank = true;
        } else {
            lines.push(line);
            blank = false;
        }
    }
    lines.join("\n").trim().to_owned()
}

/// applyPromptToMarkdown truncates at 100,000 UTF-16 code units and
/// advertises truncation rather than silently discarding the tail.
/// Rust strings cannot contain lone UTF-16 surrogates, so retain the last
/// complete Unicode scalar at the boundary.
fn truncate_source_markdown(text: &str) -> String {
    if text.encode_utf16().count() <= MAX_MARKDOWN_UTF16 {
        return text.to_owned();
    }
    let mut length = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let units = ch.len_utf16();
        if length + units > MAX_MARKDOWN_UTF16 {
            break;
        }
        out.push(ch);
        length += units;
    }
    out.push_str("\n\n[Content truncated due to length...]");
    out
}

async fn fetch_with_redirect_policy_in_workspace(
    url: &str,
    allow_private_for_test: bool,
    approve_redirect: &(impl Fn(&Url) -> Result<()> + Sync),
    workspace: Option<&Path>,
) -> Result<Value> {
    // Cache successful results under the requested URL, scoped to workspace
    // because a binary attachment path is only valid in its own workspace.
    let cache_key = workspace.map(|root| format!("{}\0{url}", root.display()));
    if let Some(value) = cache_key.as_deref().and_then(cached_page) {
        return Ok(value);
    }
    if url.encode_utf16().count() > 2_000 {
        bail!("WebFetch URL exceeds the original 2,000-character limit");
    }
    let mut current = Url::parse(url)?;
    if !current.host_str().is_some_and(|host| host.contains('.')) {
        bail!("WebFetch requires a publicly resolvable host");
    }
    // The original getURLMarkdownContent upgrades plain HTTP before its
    // initial request. The private-address override exists only for local
    // transport tests, where a TLS listener is not available.
    if !allow_private_for_test && current.scheme() == "http" {
        current
            .set_scheme("https")
            .map_err(|_| anyhow::anyhow!("Cannot upgrade WebFetch URL to HTTPS"))?;
    }
    for _ in 0..=10 {
        let mut builder = crate::tls_ca::client_builder()?
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("AtlasAgent/0.1 (+user-requested-web-fetch)");
        if !allow_private_for_test {
            let addresses = verify_public(&current).await?;
            let host = current.host_str().context("URL missing host")?;
            // Pin the validated address while preserving Host and TLS SNI.
            builder = builder.resolve(host, addresses[0]);
        }
        let client = builder.build()?;
        let mut response = client
            .get(current.clone())
            .header(reqwest::header::ACCEPT, "text/markdown, text/html, */*")
            .send()
            .await
            .with_context(|| format!("GET {current}"))?;
        if matches!(response.status().as_u16(), 301 | 302 | 307 | 308) {
            let location = response
                .headers()
                .get("location")
                .context("HTTP redirect missing Location")?
                .to_str()
                .context("Invalid redirect Location")?;
            let next = current.join(location)?;
            if !permitted_redirect(&current, &next) {
                // The original tool returns the destination to the agent
                // instead of silently fetching it under the old approval.
                // Existing explicit deny rules still take precedence.
                if next.host_str() != current.host_str() {
                    approve_redirect(&next)?;
                }
                let message = format!(
                    "REDIRECT DETECTED: The URL redirects to a different origin.\n\nOriginal URL: {current}\nRedirect URL: {next}\nStatus: {}\n\nFetch the redirected URL in a new web_fetch call with fresh permission approval.",
                    response.status()
                );
                return Ok(json!({
                    "url": current.as_str(),
                    "code": response.status().as_u16(),
                    "bytes": message.len(),
                    "content": message,
                    "redirectUrl": next.as_str(),
                    "redirect": true
                }));
            }
            current = next;
            continue;
        }
        let code = response.status();
        if code.as_u16() == 403
            && response
                .headers()
                .get("x-proxy-error")
                .and_then(|header| header.to_str().ok())
                == Some("blocked-by-allowlist")
        {
            bail!(
                "WebFetch egress policy blocked access to {}",
                current.host_str().unwrap_or("")
            );
        }
        if !code.is_success() {
            bail!("HTTP {code} at {current}")
        }
        let kind = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        // The original WebFetch persists binary responses for later inspection
        // while retaining a lossy textual representation for model analysis.
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
        let persisted = if crate::mcp_output_storage::is_binary_content_type(&kind) {
            workspace.and_then(|root| {
                match crate::mcp_output_storage::persist_binary_content(
                    root,
                    &bytes,
                    Some(&kind),
                    &format!("webfetch-{}", Uuid::new_v4()),
                ) {
                    Ok(path) => Some(path),
                    Err(err) => {
                        eprintln!("WebFetch binary attachment could not be saved: {err:#}");
                        None
                    }
                }
            })
        } else {
            None
        };
        let mut content = truncate_source_markdown(&text);
        if let Some(saved) = persisted.as_ref() {
            content.push_str("\n\n");
            content.push_str(&crate::mcp_output_storage::get_binary_blob_saved_message(
                &saved.filepath.to_string_lossy(),
                Some(&kind),
                saved.size,
                "",
            ));
        }
        let mut output = json!({
            "url":current.as_str(),"code":code.as_u16(),"bytes":bytes.len(),
            "content":content,"contentType":kind,
            "codeText":code.canonical_reason().unwrap_or("")
        });
        if let Some(saved) = persisted {
            output["persistedPath"] = json!(saved.filepath.to_string_lossy().to_string());
            output["persistedSize"] = json!(saved.size);
        }
        if let Some(key) = cache_key.as_ref() {
            cache_page(key.clone(), output.clone(), text.len());
        }
        return Ok(output);
    }
    bail!("Too many redirects (exceeded 10)")
}

async fn fetch_with_redirect_policy(
    url: &str,
    allow_private_for_test: bool,
    approve_redirect: &(impl Fn(&Url) -> Result<()> + Sync),
) -> Result<Value> {
    fetch_with_redirect_policy_in_workspace(url, allow_private_for_test, approve_redirect, None)
        .await
}

async fn fetch(url: &str, allow_private_for_test: bool) -> Result<Value> {
    fetch_with_redirect_policy(url, allow_private_for_test, &|_| Ok(())).await
}

pub async fn execute_with_redirect_policy_in_workspace(
    args: &Value,
    workspace: &Path,
    approve_redirect: &(impl Fn(&Url) -> Result<()> + Sync),
) -> Result<String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .context("web_fetch requires url")?;
    let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or("");
    let started = Instant::now();
    let mut output =
        fetch_with_redirect_policy_in_workspace(url, false, approve_redirect, Some(workspace))
            .await?;
    if output.get("redirect").and_then(Value::as_bool) == Some(true) {
        // The original tool tells the agent exactly which URL and original
        // prompt to use on the next, separately authorized fetch.
        if let Some(target) = output.get("redirectUrl").and_then(Value::as_str) {
            let previous = output.get("content").and_then(Value::as_str).unwrap_or("");
            let message = format!(
                "{previous}\n\nTo complete the request, use web_fetch again with fresh approval:\n- url: {target:?}\n- prompt: {prompt:?}"
            );
            output["bytes"] = json!(message.len());
            output["content"] = json!(message);
        }
    }
    output["prompt"] = json!(prompt);
    output["durationMs"] = json!(started.elapsed().as_millis());
    Ok(output.to_string())
}

pub async fn execute_with_redirect_policy(
    args: &Value,
    approve_redirect: &(impl Fn(&Url) -> Result<()> + Sync),
) -> Result<String> {
    let root = std::env::current_dir()?;
    execute_with_redirect_policy_in_workspace(args, &root, approve_redirect).await
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
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "febf::1",
        ] {
            assert!(!acceptable_ip(host.parse().unwrap()), "{host}");
        }
        assert!(acceptable_ip("8.8.8.8".parse().unwrap()));
        assert!(acceptable_ip("2001:4860:4860::8888".parse().unwrap()));
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
            crate::test_http::consume_request(&mut stream)
                .await
                .unwrap();
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
    async fn mixed_case_binary_response_is_saved_without_loss() {
        use tokio::io::AsyncWriteExt;
        let root = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            crate::test_http::consume_request(&mut stream)
                .await
                .unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: APPLICATION/PDF; charset=binary\r\nContent-Length: 6\r\nConnection: close\r\n\r\n%PDF\0\xff").await.unwrap();
        });
        let result = fetch_with_redirect_policy_in_workspace(
            &format!("http://{address}/file"),
            true,
            &|_| Ok(()),
            Some(root.path()),
        )
        .await
        .unwrap();
        server.await.unwrap();
        let saved = Path::new(result["persistedPath"].as_str().unwrap());
        assert_eq!(saved.extension().unwrap(), "pdf");
        assert_eq!(std::fs::read(saved).unwrap(), b"%PDF\0\xff");
        assert_eq!(result["persistedSize"], 6);
        assert!(result["content"].as_str().unwrap().contains("6 bytes"));
    }
    #[tokio::test]
    async fn web_fetch_reads_local_mock_only_with_internal_test_override() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            crate::test_http::consume_request(&mut stream)
                .await
                .unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 18\r\nConnection: close\r\n\r\n<h1>Web hello</h1>").await.unwrap();
        });
        let url = format!("http://{address}/test");
        assert!(verify_public(&Url::parse(&url).unwrap()).await.is_err());
        let result = fetch(&url, true).await.unwrap();
        server.await.unwrap();
        assert_eq!(result["code"], 200);
        assert_eq!(result["content"], "# Web hello");
    }
}
