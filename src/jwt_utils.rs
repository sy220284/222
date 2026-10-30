//! Minimal JWT decoding helpers for session-ingress credentials.
//! 2.1.88 reference: `src/bridge/jwtUtils.ts` (decodeJwtPayload /
//! decodeJwtExpiry / TokenRefreshScheduler).
//!
//! This port only *decodes* JWTs — it never signs or contacts the original
//! vendor endpoints. Refresh scheduling drives self-hosted endpoints only.

use base64::Engine;
use serde_json::Value;

/// Decode the JWT payload section without signature verification.
/// Mirrors `decodeJwtPayload`: JSON parse failure yields None.
pub fn decode_jwt_payload(token: &str) -> Option<Value> {
    let jwt = token.strip_prefix("sk-ant-si-").unwrap_or(token);
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 || parts[1].is_empty() { return None; }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1].trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Decode the `exp` claim (seconds since epoch) from a JWT.
/// Mirrors `decodeJwtExpiry`.
pub fn decode_jwt_expiry(token: &str) -> Option<u64> {
    let payload = decode_jwt_payload(token)?;
    payload.get("exp")?.as_u64()
}

/// Seconds before expiry at which refresh is scheduled (source: 60_000).
pub const JWT_REFRESH_BUFFER_MS: u64 = 300_000;

/// Minimum delay between refresh attempts (source: 30_000).
pub const MIN_REFRESH_DELAY_MS: u64 = 30_000;

/// Consecutive refresh failures tolerated before the chain gives up (source: 3).
pub const MAX_CONSECUTIVE_REFRESH_FAILURES: u32 = 3;

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::json;

    fn b64url(value: &Value) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn decodes_payload_and_expiry() {
        let token = format!(
            "{}.{}.sig",
            b64url(&json!({"alg":"HS256"})),
            b64url(&json!({"exp":1_700_000_000_u64,"sid":"abc"}))
        );
        assert_eq!(decode_jwt_expiry(&token), Some(1_700_000_000));
        let payload = decode_jwt_payload(&token).unwrap();
        assert_eq!(payload["sid"], "abc");
    }

    #[test]
    fn string_expires_are_rejected() {
        let token = format!("x.{}.y", b64url(&json!({"exp":"123"})));
        assert_eq!(decode_jwt_expiry(&token), None);
    }

    #[test]
    fn garbage_tokens_return_none() {
        assert_eq!(decode_jwt_expiry("not-a-jwt"), None);
        assert_eq!(decode_jwt_payload("a.b.c"), None);
    }
}
