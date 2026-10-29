//! Provider-neutral OAuth client implementing the behavior of 2.1.88
//! src/services/oauth/client.ts and the application-neutral parts of index.ts.
//! Authentication endpoints, scopes and redirect URIs are provider-specific
//! configuration rather than vendor-coupled constants.
use anyhow::{bail, Context, Result};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub authorization_url: Url,
    pub token_url: Url,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub manual_redirect_uri: String,
    pub callback_path: String,
    pub roles_url: Option<Url>,
    pub api_key_url: Option<Url>,
}

impl OAuthProvider {
    pub fn new(
        authorization_url: &str,
        token_url: &str,
        client_id: &str,
        scopes: Vec<String>,
        manual_redirect_uri: &str,
    ) -> Result<Self> {
        let authorization_url = Url::parse(authorization_url)?;
        let token_url = Url::parse(token_url)?;
        for url in [&authorization_url, &token_url] {
            if !matches!(url.scheme(), "https" | "http") {
                bail!("OAuth endpoints must use HTTP or HTTPS");
            }
            if !url.username().is_empty() || url.password().is_some() {
                bail!("OAuth endpoints may not embed credentials");
            }
            if url.scheme() == "http" && !url.host_str().is_some_and(|h| {
                h == "localhost" || h == "127.0.0.1" || h == "::1"
            }) {
                bail!("Insecure OAuth HTTP endpoints are allowed only on localhost");
            }
        }
        if client_id.trim().is_empty() {
            bail!("OAuth client ID must not be empty");
        }
        if scopes.is_empty() || scopes.iter().any(|s| s.is_empty()) {
            bail!("OAuth provider must specify nonempty scopes");
        }
        let manual = Url::parse(manual_redirect_uri)?;
        if !matches!(manual.scheme(), "https" | "http") {
            bail!("Manual redirect URI must be HTTP(S)");
        }
        Ok(Self {
            authorization_url,
            token_url,
            client_id: client_id.to_owned(),
            scopes,
            manual_redirect_uri: manual_redirect_uri.to_owned(),
            callback_path: "/callback".into(),
            roles_url: None,
            api_key_url: None,
        })
    }

    pub fn automatic_redirect_uri(&self, port: u16) -> String {
        format!("http://localhost:{port}{}", self.callback_path)
    }

    pub fn build_auth_url(
        &self,
        challenge: &str,
        state: &str,
        redirect_uri: &str,
        options: &AuthorizationOptions,
    ) -> Url {
        let mut url = self.authorization_url.clone();
        {
            let mut params = url.query_pairs_mut();
            params.append_pair("client_id", &self.client_id);
            params.append_pair("response_type", "code");
            params.append_pair("redirect_uri", redirect_uri);
            params.append_pair("scope", &self.scopes.join(" "));
            params.append_pair("code_challenge", challenge);
            params.append_pair("code_challenge_method", "S256");
            params.append_pair("state", state);
            if let Some(org) = options.organization.as_deref().filter(|s| !s.is_empty()) {
                params.append_pair("orgUUID", org);
            }
            if let Some(hint) = options.login_hint.as_deref().filter(|s| !s.is_empty()) {
                params.append_pair("login_hint", hint);
            }
            if let Some(method) = options.login_method.as_deref().filter(|s| !s.is_empty()) {
                params.append_pair("login_method", method);
            }
        }
        url
    }

    pub fn authorization_links(
        &self,
        pkce: &crate::oauth_crypto::PkceRequest,
        port: u16,
        options: &AuthorizationOptions,
    ) -> AuthorizationLinks {
        AuthorizationLinks {
            manual: self.build_auth_url(
                &pkce.challenge, &pkce.state, &self.manual_redirect_uri, options
            ).to_string(),
            automatic: self.build_auth_url(
                &pkce.challenge, &pkce.state, &self.automatic_redirect_uri(port), options
            ).to_string(),
        }
    }

    fn http_client() -> Result<Client> {
        Ok(Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?)
    }

    pub async fn exchange_code_for_tokens(
        &self,
        code: &str,
        state: &str,
        verifier: &str,
        redirect_uri: &str,
        expires_in: Option<u64>,
    ) -> Result<OAuthTokens> {
        if code.is_empty() || state.is_empty() || verifier.is_empty() {
            bail!("OAuth code, state and verifier are required");
        }
        let mut body = json!({
            "grant_type":"authorization_code",
            "code":code,
            "redirect_uri":redirect_uri,
            "client_id":self.client_id,
            "code_verifier":verifier,
            "state":state
        });
        if let Some(seconds) = expires_in {
            body["expires_in"] = json!(seconds);
        }
        let response = Self::http_client()?
            .post(self.token_url.clone())
            .json(&body)
            .send()
            .await
            .context("OAuth authorization-code exchange failed")?;
        if response.status() != reqwest::StatusCode::OK {
            bail!("OAuth token exchange failed (HTTP {})", response.status().as_u16());
        }
        let token: TokenExchange = response.json().await
            .context("OAuth token endpoint returned invalid JSON")?;
        OAuthTokens::from_exchange(token, None)
    }

    pub async fn refresh_oauth_token(
        &self,
        refresh_token: &str,
        requested_scopes: Option<&[String]>,
    ) -> Result<OAuthTokens> {
        if refresh_token.is_empty() {
            bail!("Cannot refresh without a refresh token");
        }
        let scopes = requested_scopes.filter(|s| !s.is_empty()).unwrap_or(&self.scopes);
        let request_body = json!({
            "grant_type":"refresh_token",
            "refresh_token":refresh_token,
            "client_id":self.client_id,
            "scope":scopes.join(" ")
        });
        let response = Self::http_client()?
            .post(self.token_url.clone())
            .json(&request_body)
            .send()
            .await
            .context("OAuth token refresh request failed")?;
        if response.status() != reqwest::StatusCode::OK {
            bail!("OAuth token refresh failed (HTTP {})", response.status().as_u16());
        }
        let token: TokenExchange = response.json().await
            .context("OAuth refresh endpoint returned invalid JSON")?;
        OAuthTokens::from_exchange(token, Some(refresh_token))
    }

    pub async fn fetch_user_roles(&self, access_token: &str) -> Result<Value> {
        let url = self.roles_url.as_ref().context("Provider has no roles endpoint")?;
        let response = Self::http_client()?
            .get(url.clone())
            .bearer_auth(access_token)
            .send()
            .await
            .context("OAuth roles request failed")?;
        if response.status() != reqwest::StatusCode::OK {
            bail!("OAuth roles endpoint returned HTTP {}", response.status().as_u16());
        }
        response.json().await.context("Invalid roles response")
    }

    pub async fn create_api_key(&self, access_token: &str) -> Result<Option<String>> {
        let url = self.api_key_url.as_ref().context("Provider has no API key endpoint")?;
        let response = Self::http_client()?
            .post(url.clone())
            .bearer_auth(access_token)
            .send()
            .await
            .context("OAuth API key creation failed")?;
        if !response.status().is_success() {
            bail!("OAuth API key endpoint returned HTTP {}", response.status().as_u16());
        }
        let body: Value = response.json().await.context("Invalid API key response")?;
        Ok(body["raw_key"].as_str().filter(|s| !s.is_empty()).map(str::to_owned))
    }
}

#[derive(Debug, Default, Clone)]
pub struct AuthorizationOptions {
    pub organization: Option<String>,
    pub login_hint: Option<String>,
    pub login_method: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationLinks {
    pub automatic: String,
    pub manual: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenExchange {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    account: Option<Value>,
    #[serde(default)]
    organization: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: u64,
    pub scopes: Vec<String>,
    pub account: Option<Value>,
    pub organization: Option<Value>,
}

impl OAuthTokens {
    fn from_exchange(token: TokenExchange, existing_refresh: Option<&str>) -> Result<Self> {
        if token.access_token.is_empty() {
            bail!("OAuth token exchange returned no access token");
        }
        let refresh_token = token.refresh_token
            .filter(|value| !value.is_empty())
            .or_else(|| existing_refresh.map(str::to_owned))
            .context("OAuth token exchange returned no refresh token")?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
        let expiry = now.saturating_add((token.expires_in as u128).saturating_mul(1000));
        let expires_at_ms = u64::try_from(expiry).context("Invalid OAuth expiry")?;
        Ok(Self {
            access_token: token.access_token,
            refresh_token,
            expires_at_ms,
            scopes: parse_scopes(&token.scope),
            account: token.account,
            organization: token.organization,
        })
    }

    pub fn is_expired_with_buffer(&self) -> bool {
        is_oauth_token_expired(Some(self.expires_at_ms))
    }
}

pub fn parse_scopes(scopes: &str) -> Vec<String> {
    scopes.split(' ').filter(|part| !part.is_empty()).map(str::to_owned).collect()
}

/// Source semantics: unknown expiry does not imply expiration; a known expiry
/// within five minutes is treated as expired to avoid racing a request.
pub fn is_oauth_token_expired(expires_at_ms: Option<u64>) -> bool {
    let Some(expiry) = expires_at_ms else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    now.saturating_add(300_000) >= u128::from(expiry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

    async fn respond(body: &str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let body = body.to_owned();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            loop {
                let mut block = [0u8; 1024];
                let size = stream.read(&mut block).await.unwrap();
                assert_ne!(size, 0);
                buffer.extend_from_slice(&block[..size]);
                if let Some(end) = buffer.windows(4).position(|part| part == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&buffer[..end+4]);
                    let content_length = header.lines()
                        .find_map(|line| {
                            let lowercase = line.to_ascii_lowercase();
                            lowercase.strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    while buffer.len() < end + 4 + content_length {
                        let read = stream.read(&mut block).await.unwrap();
                        assert_ne!(read, 0);
                        buffer.extend_from_slice(&block[..read]);
                    }
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(buffer).unwrap()
        });
        (url, handle)
    }

    fn provider(base: &str) -> OAuthProvider {
        OAuthProvider::new(
            &format!("{base}/authorize"),
            &format!("{base}/token"),
            "public-client",
            vec!["openid".into(), "profile".into()],
            "http://localhost:8787/manual",
        ).unwrap()
    }

    #[test]
    fn auth_urls_include_pkce_and_provider_scopes_with_distinct_redirects() {
        let config = provider("http://127.0.0.1:9999");
        let pkce = crate::oauth_crypto::PkceRequest::create().unwrap();
        let urls = config.authorization_links(&pkce, 35000, &AuthorizationOptions {
            login_hint: Some("a@example.org".into()),
            organization: Some("team-1".into()),
            login_method: Some("sso".into()),
        });
        let auto = Url::parse(&urls.automatic).unwrap();
        let manual = Url::parse(&urls.manual).unwrap();
        let pairs: std::collections::HashMap<_,_> = auto.query_pairs().into_owned().collect();
        assert_eq!(pairs["code_challenge"], pkce.challenge);
        assert_eq!(pairs["state"], pkce.state);
        assert_eq!(pairs["scope"], "openid profile");
        assert_eq!(pairs["redirect_uri"], "http://localhost:35000/callback");
        assert_eq!(pairs["login_hint"], "a@example.org");
        assert_eq!(pairs["orgUUID"], "team-1");
        assert_eq!(pairs["login_method"], "sso");
        assert_eq!(manual.query_pairs().find(|(k,_)| k=="redirect_uri").unwrap().1, "http://localhost:8787/manual");
    }

    #[tokio::test]
    async fn code_exchange_and_refresh_preserve_rotated_tokens_and_expiry() {
        let (base, exchange) = respond(r#"{"access_token":"token-one","refresh_token":"refresh-one","expires_in":3600,"scope":"openid profile"}"#).await;
        let config = provider(&base);
        let first = config.exchange_code_for_tokens(
            "auth code", "state", "verifier", "http://localhost:35000/callback", Some(3600)
        ).await.unwrap();
        assert_eq!(first.access_token, "token-one");
        assert_eq!(first.refresh_token, "refresh-one");
        assert_eq!(first.scopes, ["openid", "profile"]);
        assert!(!first.is_expired_with_buffer());
        let exchange_request = exchange.await.unwrap();
        assert!(exchange_request.starts_with("POST /token"));
        let body = exchange_request.split("\r\n\r\n").nth(1).unwrap();
        let posted: Value = serde_json::from_str(body).unwrap();
        assert_eq!(posted["code"], "auth code");
        assert_eq!(posted["code_verifier"], "verifier");
        assert_eq!(posted["expires_in"], 3600);

        let (base, refresh) = respond(r#"{"access_token":"token-two","expires_in":600,"scope":"profile"}"#).await;
        let config = provider(&base);
        let second = config.refresh_oauth_token("refresh-one", None).await.unwrap();
        assert_eq!(second.access_token, "token-two");
        assert_eq!(second.refresh_token, "refresh-one");
        assert_eq!(second.scopes, ["profile"]);
        let posted: Value = serde_json::from_str(refresh.await.unwrap().split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(posted["grant_type"], "refresh_token");
        assert_eq!(posted["refresh_token"], "refresh-one");
        assert_eq!(posted["scope"], "openid profile");
    }

    #[test]
    fn expiry_buffer_and_provider_security_rules() {
        assert!(!is_oauth_token_expired(None));
        assert!(is_oauth_token_expired(Some(0)));
        assert!(OAuthProvider::new(
            "http://example.org/authorize", "https://example.org/token", "x",
            vec!["profile".into()], "https://example.org/manual"
        ).is_err());
        assert!(OAuthProvider::new(
            "https://user:secret@example.org/authorize", "https://example.org/token", "x",
            vec!["profile".into()], "https://example.org/manual"
        ).is_err());
    }
}
