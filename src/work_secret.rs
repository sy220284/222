//! Work-secret decoding and session URL construction for the self-hosted
//! remote bridge. 2.1.88 reference: `src/bridge/workSecret.ts`.
//!
//! The Rust port keeps the wire-compatible helpers (decode/build/compare) and
//! `register_worker`; the worker registers against *self-hosted* endpoints
//! only — the original vendor base URLs are never contacted.

use anyhow::{bail, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

/// A decoded `version: 1` work secret.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WorkSecret {
    #[serde(default = "default_version")]
    pub version: u64,
    /// Bearer token for the session-ingress service.
    pub session_ingress_token: String,
    /// Base URL of the (self-hosted) API gateway.
    pub api_base_url: String,
}

fn default_version() -> u64 {
    1
}

/// Decode a base64url-encoded work secret and validate its version.
/// Mirrors `decodeWorkSecret`.
pub fn decode_work_secret(secret: &str) -> Result<WorkSecret> {
    let normalized = secret.trim_end_matches('=').replace('-', "+").replace('_', "/");
    let json = base64::engine::general_purpose::STANDARD
        .decode(normalized.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .or_else(|| String::from_utf8(secret.as_bytes().to_vec()).ok());
    let parsed: Value = match json {
        Some(text) => serde_json::from_str(&text)
            .or_else(|_| serde_json::from_str(secret))?,
        None => bail!("Unsupported work secret version: unknown"),
    };
    let version = parsed.get("version").and_then(Value::as_u64);
    if version != Some(1) {
        bail!(
            "Unsupported work secret version: {}",
            version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );
    }
    let token = parsed
        .get("session_ingress_token")
        .and_then(Value::as_str)
        .unwrap_or("");
    if token.is_empty() {
        bail!("Invalid work secret: missing or empty session_ingress_token");
    }
    if parsed.get("api_base_url").and_then(Value::as_str).is_none() {
        bail!("Invalid work secret: missing api_base_url");
    }
    serde_json::from_value(parsed)
        .map_err(|error| anyhow::anyhow!("Invalid work secret: {error}"))
}

/// Build a WebSocket SDK URL from the API base URL and session ID.
/// Mirrors `buildSdkUrl`: `/v2/` for localhost (direct ingress) and `/v1/`
/// otherwise, with `ws://` only for plaintext local testing.
pub fn build_sdk_url(api_base_url: &str, session_id: &str) -> String {
    let is_localhost =
        api_base_url.contains("localhost") || api_base_url.contains("127.0.0.1");
    let protocol = if is_localhost { "ws" } else { "wss" };
    let version = if is_localhost { "v2" } else { "v1" };
    let host = api_base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    format!("{protocol}://{host}/{version}/session_ingress/ws/{session_id}")
}

/// Compare two session IDs regardless of their tagged-ID prefix.
/// Mirrors `sameSessionId`: bodies after the last underscore must match
/// (minimum 4 chars to avoid short-suffix accidents).
pub fn same_session_id(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_body = a.rsplit('_').next().unwrap_or("");
    let b_body = b.rsplit('_').next().unwrap_or("");
    a_body.len() >= 4 && a_body == b_body
}

/// Build a CCR v2 session URL (HTTP, not ws) pointing at
/// `/v1/code/sessions/{id}`. Mirrors `buildCCRv2SdkUrl`.
pub fn build_ccr_v2_sdk_url(api_base_url: &str, session_id: &str) -> String {
    let base = api_base_url.trim_end_matches('/');
    format!("{base}/v1/code/sessions/{session_id}")
}

/// Register this bridge as the worker for a CCR v2 session and return the
/// worker epoch. Mirrors `registerWorker` (POST `{sessionUrl}/worker/register`).
/// `register` is injectable for loopback tests.
pub async fn register_worker_with<F, Fut>(
    session_url: &str,
    access_token: &str,
    register: F,
) -> Result<u64>
where
    F: FnOnce(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<Value>>,
{
    let response = register(
        format!("{session_url}/worker/register"),
        format!("Bearer {access_token}"),
    )
    .await?;
    // protojson serializes int64 as a string; Go may also return a number.
    let raw = response.get("worker_epoch").cloned();
    let epoch = match raw {
        Some(Value::String(text)) => text.parse::<u64>().ok(),
        Some(Value::Number(number)) => number.as_u64(),
        _ => None,
    };
    match epoch {
        Some(epoch) => Ok(epoch),
        None => bail!(
            "registerWorker: invalid worker_epoch in response: {}",
            serde_json::to_string(&response).unwrap_or_default()
        ),
    }
}

/// Production registration path (self-hosted endpoint only).
pub async fn register_worker(
    session_url: &str,
    access_token: &str,
) -> Result<u64> {
    register_worker_with(session_url, access_token, |url, auth| async move {
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(10))
            .json(&json!({}))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            bail!("registerWorker: POST failed with {status}");
        }
        Ok(response.json::<Value>().await?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn encode_secret(value: &Value) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn decodes_version_one_secret() {
        let secret = encode_secret(&json!({
            "version": 1,
            "session_ingress_token": "tok",
            "api_base_url": "https://bridge.example.internal"
        }));
        let decoded = decode_work_secret(&secret).unwrap();
        assert_eq!(decoded.session_ingress_token, "tok");
        assert_eq!(decoded.api_base_url, "https://bridge.example.internal");
    }

    #[test]
    fn rejects_other_versions_and_missing_fields() {
        let v2 = encode_secret(&json!({"version":2,"session_ingress_token":"t","api_base_url":"u"}));
        assert!(decode_work_secret(&v2)
            .unwrap_err()
            .to_string()
            .contains("Unsupported work secret version: 2"));
        let no_token = encode_secret(&json!({"version":1,"api_base_url":"u"}));
        assert!(decode_work_secret(&no_token)
            .unwrap_err()
            .to_string()
            .contains("session_ingress_token"));
        let no_url = encode_secret(&json!({"version":1,"session_ingress_token":"t"}));
        assert!(decode_work_secret(&no_url)
            .unwrap_err()
            .to_string()
            .contains("api_base_url"));
    }

    #[test]
    fn sdk_urls_follow_localhost_rules() {
        assert_eq!(
            build_sdk_url("http://localhost:3000", "s1"),
            "ws://localhost:3000/v2/session_ingress/ws/s1"
        );
        assert_eq!(
            build_sdk_url("https://bridge.example.internal/", "s1"),
            "wss://bridge.example.internal/v1/session_ingress/ws/s1"
        );
        assert_eq!(
            build_ccr_v2_sdk_url("https://bridge.example.internal", "s1"),
            "https://bridge.example.internal/v1/code/sessions/s1"
        );
    }

    #[test]
    fn tagged_session_ids_compare_by_body() {
        assert!(same_session_id("session_abc", "session_abc"));
        assert!(same_session_id("session_12345678", "cse_12345678"));
        assert!(same_session_id("session_staging_12345678", "cse_12345678"));
        assert!(!same_session_id("session_12345678", "cse_87654321"));
        assert!(!same_session_id("session_ab", "cse_ab"));
    }

    #[tokio::test]
    async fn register_worker_parses_string_and_number_epochs() {
        let epoch = register_worker_with("https://x/s1", "tok", |url, auth| async move {
            assert_eq!(url, "https://x/s1/worker/register");
            assert_eq!(auth, "Bearer tok");
            Ok(json!({"worker_epoch": "42"}))
        })
        .await
        .unwrap();
        assert_eq!(epoch, 42);
        let epoch = register_worker_with("u", "t", |_u, _a| async move {
            Ok(json!({"worker_epoch": 7}))
        })
        .await
        .unwrap();
        assert_eq!(epoch, 7);
        let error = register_worker_with("u", "t", |_u, _a| async move {
            Ok(json!({"worker_epoch": "NaN"}))
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("invalid worker_epoch"));
    }
}
