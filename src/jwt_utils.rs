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
    if parts.len() != 3 || parts[1].is_empty() {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1].trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Decode the `exp` claim (seconds since epoch) from a JWT.
/// Mirrors `decodeJwtExpiry`.
pub fn decode_jwt_expiry(token: &str) -> Option<u64> {
    let payload = decode_jwt_payload(token)?;
    payload.get("exp")?.as_u64()
}

/// Refresh five minutes before a decodable JWT expires.
pub const JWT_REFRESH_BUFFER_MS: u64 = 5 * 60 * 1_000;
/// An opaque JWT or a refreshed OAuth token gets a 30-minute follow-up.
pub const FALLBACK_REFRESH_INTERVAL_MS: u64 = 30 * 60 * 1_000;
/// A missing access token is retried once per minute.
pub const REFRESH_RETRY_DELAY_MS: u64 = 60_000;
/// Explicit expires_in refresh delays have a 30-second floor.
pub const MIN_REFRESH_DELAY_MS: u64 = 30_000;
/// Stop after three consecutive failures to retrieve an access token.
pub const MAX_CONSECUTIVE_REFRESH_FAILURES: u32 = 3;

/// The refresh source and delivery callback are deliberately protocol-neutral:
/// a self-hosted bridge can inject its token provider and transport sender.
type AccessTokenFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Option<String>>> + Send>>;
type AccessTokenProvider = dyn Fn() -> AccessTokenFuture + Send + Sync;
type RefreshDelivery = dyn Fn(&str, &str) + Send + Sync;

struct RefreshTimer {
    generation: u64,
    failures: u32,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
struct RefreshState {
    next_generation: u64,
    sessions: std::collections::HashMap<String, RefreshTimer>,
}

struct RefreshInner {
    state: std::sync::Mutex<RefreshState>,
    access_token: Box<AccessTokenProvider>,
    deliver: Box<RefreshDelivery>,
    buffer: std::time::Duration,
}

/// Per-session JWT refresh lifecycle equivalent to createTokenRefreshScheduler.
///
/// Replacing or cancelling a session aborts its pending timer and invalidates
/// any refresh that was awaiting a token. An opaque token leaves its existing
/// refresh chain intact, matching the original source.
#[derive(Clone)]
pub struct TokenRefreshScheduler {
    inner: std::sync::Arc<RefreshInner>,
}

impl TokenRefreshScheduler {
    pub fn new<F, Fut, D>(get_access_token: F, on_refresh: D) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = anyhow::Result<Option<String>>> + Send + 'static,
        D: Fn(&str, &str) + Send + Sync + 'static,
    {
        Self::with_refresh_buffer(
            get_access_token,
            on_refresh,
            std::time::Duration::from_millis(JWT_REFRESH_BUFFER_MS),
        )
    }

    pub fn with_refresh_buffer<F, Fut, D>(
        get_access_token: F,
        on_refresh: D,
        buffer: std::time::Duration,
    ) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = anyhow::Result<Option<String>>> + Send + 'static,
        D: Fn(&str, &str) + Send + Sync + 'static,
    {
        Self {
            inner: std::sync::Arc::new(RefreshInner {
                state: std::sync::Mutex::new(RefreshState::default()),
                access_token: Box::new(move || Box::pin(get_access_token())),
                deliver: Box::new(on_refresh),
                buffer,
            }),
        }
    }

    /// False means the token has no usable expiry; existing timers are kept.
    pub fn schedule(&self, session_id: &str, token: &str) -> bool {
        let Some(expiry) = decode_jwt_expiry(token).filter(|exp| *exp != 0) else {
            return false;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let remaining = u128::from(expiry)
            .saturating_mul(1_000)
            .saturating_sub(now)
            .saturating_sub(self.inner.buffer.as_millis());
        let delay_ms = remaining.min(u128::from(u64::MAX)) as u64;
        self.schedule_timer(session_id, std::time::Duration::from_millis(delay_ms));
        true
    }

    /// Same 30-second lower bound as the original scheduleFromExpiresIn.
    pub fn schedule_from_expires_in(&self, session_id: &str, expires_in_seconds: i64) {
        let millis = i128::from(expires_in_seconds)
            .saturating_mul(1_000)
            .saturating_sub(self.inner.buffer.as_millis().min(i128::MAX as u128) as i128)
            .max(i128::from(MIN_REFRESH_DELAY_MS))
            .min(i128::from(u64::MAX)) as u64;
        self.schedule_timer(session_id, std::time::Duration::from_millis(millis));
    }

    fn schedule_timer(&self, session_id: &str, delay: std::time::Duration) {
        let mut state = self.inner.state.lock().expect("token refresh state");
        state.next_generation = state
            .next_generation
            .checked_add(1)
            .expect("token refresh generation exhausted");
        let generation = state.next_generation;
        let handle = Self::spawn_timer(
            std::sync::Arc::downgrade(&self.inner),
            session_id.to_owned(),
            generation,
            delay,
        );
        if let Some(previous) = state.sessions.insert(
            session_id.to_owned(),
            RefreshTimer {
                generation,
                failures: 0,
                handle,
            },
        ) {
            previous.handle.abort();
        }
    }

    fn spawn_timer(
        weak: std::sync::Weak<RefreshInner>,
        session_id: String,
        generation: u64,
        delay: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            if let Some(inner) = weak.upgrade() {
                Self::refresh(inner, session_id, generation).await;
            }
        })
    }

    async fn refresh(inner: std::sync::Arc<RefreshInner>, session_id: String, generation: u64) {
        let token = (inner.access_token)()
            .await
            .ok()
            .flatten()
            .filter(|v| !v.is_empty());
        if let Some(token) = token {
            {
                let mut state = inner.state.lock().expect("token refresh state");
                let Some(entry) = state.sessions.get_mut(&session_id) else {
                    return;
                };
                if entry.generation != generation {
                    return;
                }
                entry.failures = 0;
            }
            (inner.deliver)(&session_id, &token);
            let mut state = inner.state.lock().expect("token refresh state");
            if let Some(entry) = state.sessions.get_mut(&session_id) {
                if entry.generation == generation {
                    entry.handle = Self::spawn_timer(
                        std::sync::Arc::downgrade(&inner),
                        session_id.clone(),
                        generation,
                        std::time::Duration::from_millis(FALLBACK_REFRESH_INTERVAL_MS),
                    );
                }
            }
        } else {
            let mut state = inner.state.lock().expect("token refresh state");
            if let Some(entry) = state.sessions.get_mut(&session_id) {
                if entry.generation != generation {
                    return;
                }
                entry.failures = entry.failures.saturating_add(1);
                if entry.failures < MAX_CONSECUTIVE_REFRESH_FAILURES {
                    entry.handle = Self::spawn_timer(
                        std::sync::Arc::downgrade(&inner),
                        session_id.clone(),
                        generation,
                        std::time::Duration::from_millis(REFRESH_RETRY_DELAY_MS),
                    );
                }
            }
        }
    }

    pub fn cancel(&self, session_id: &str) {
        if let Some(entry) = self
            .inner
            .state
            .lock()
            .expect("token refresh state")
            .sessions
            .remove(session_id)
        {
            entry.handle.abort();
        }
    }

    pub fn cancel_all(&self) {
        let mut state = self.inner.state.lock().expect("token refresh state");
        for (_, entry) in state.sessions.drain() {
            entry.handle.abort();
        }
    }
}

impl Drop for TokenRefreshScheduler {
    fn drop(&mut self) {
        if std::sync::Arc::strong_count(&self.inner) == 1 {
            self.cancel_all();
        }
    }
}

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

    #[tokio::test]
    async fn refreshes_immediately_and_opaque_token_does_not_break_chain() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let delivered = refresh_count.clone();
        let scheduler = TokenRefreshScheduler::new(
            || async { Ok(Some("self-hosted-oauth-token".to_owned())) },
            move |session, token| {
                assert_eq!(session, "session-1");
                assert_eq!(token, "self-hosted-oauth-token");
                delivered.fetch_add(1, Ordering::SeqCst);
            },
        );
        let token = format!("x.{}.sig", b64url(&json!({"exp":1})));
        assert!(scheduler.schedule("session-1", &token));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while refresh_count.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!scheduler.schedule("session-1", "opaque-token"));
        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
        scheduler.cancel_all();
    }

    #[tokio::test]
    async fn cancelling_session_while_access_token_is_pending_suppresses_delivery() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let queried = Arc::new(tokio::sync::Notify::new());
        let unblock = Arc::new(tokio::sync::Notify::new());
        let received = Arc::new(AtomicUsize::new(0));
        let query_signal = queried.clone();
        let unblock_signal = unblock.clone();
        let received_signal = received.clone();
        let scheduler = TokenRefreshScheduler::new(
            move || {
                let query_signal = query_signal.clone();
                let unblock_signal = unblock_signal.clone();
                async move {
                    query_signal.notify_one();
                    unblock_signal.notified().await;
                    Ok(Some("a-token".to_owned()))
                }
            },
            move |_, _| {
                received_signal.fetch_add(1, Ordering::SeqCst);
            },
        );
        let token = format!("x.{}.sig", b64url(&json!({"exp":1})));
        assert!(scheduler.schedule("cancelled", &token));
        tokio::time::timeout(std::time::Duration::from_secs(1), queried.notified())
            .await
            .unwrap();
        scheduler.cancel("cancelled");
        unblock.notify_one();
        tokio::task::yield_now().await;
        assert_eq!(received.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn invalid_jwt_preserves_existing_session_timer() {
        let scheduler = TokenRefreshScheduler::new(|| async { Ok(None) }, |_, _| {});
        scheduler.schedule_from_expires_in("opaque", 60);
        assert!(!scheduler.schedule("opaque", "not-a-jwt"));
        let state = scheduler.inner.state.lock().unwrap();
        assert!(state.sessions.contains_key("opaque"));
        drop(state);
        scheduler.cancel_all();
    }

    #[tokio::test]
    async fn refresh_generations_are_distinct_after_rescheduling() {
        let scheduler = TokenRefreshScheduler::new(|| async { Ok(None) }, |_, _| {});
        scheduler.schedule_from_expires_in("session", 100);
        let before = scheduler.inner.state.lock().unwrap().sessions["session"].generation;
        scheduler.schedule_from_expires_in("session", 100);
        let after = scheduler.inner.state.lock().unwrap().sessions["session"].generation;
        assert!(after > before);
        scheduler.cancel_all();
    }

    #[test]
    fn garbage_tokens_return_none() {
        assert_eq!(decode_jwt_expiry("not-a-jwt"), None);
        assert_eq!(decode_jwt_payload("a.b.c"), None);
    }
}
