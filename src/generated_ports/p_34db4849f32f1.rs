//! Rust exports derived from original: src/server/createDirectConnectSession.ts
//!
//! Decision core for direct-connect sessions: the response JSON must carry
//! the server url, session id and api key; error text carries the url for
//! diagnosis. HTTP transport belongs to the server chain.

pub struct DirectConnectConfig {
    pub server_url: String,
    pub session_id: String,
    pub api_key: String,
}

/// Validate a connect response body into a usable config.
pub fn parse_connect_response(
    body: &serde_json::Value,
    fallback_server_url: &str,
) -> Result<DirectConnectConfig, String> {
    let url = body
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_server_url);
    let session_id = body
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Direct connect response missing sessionId ({url})"))?;
    let api_key = body
        .get("apiKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Direct connect response missing apiKey ({url})"))?;
    Ok(DirectConnectConfig {
        server_url: url.to_owned(),
        session_id: session_id.to_owned(),
        api_key: api_key.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn happy_and_missing_fields() {
        let cfg = parse_connect_response(&json!({"sessionId": "s1", "apiKey": "k1"}), "http://srv")
            .unwrap();
        assert_eq!(cfg.session_id, "s1");
        assert_eq!(cfg.server_url, "http://srv");
        let err = parse_connect_response(&json!({"sessionId": "s1"}), "http://srv")
            .err()
            .expect("missing secret must be rejected");
        assert!(err.contains("apiKey") && err.contains("http://srv"));
    }
}
