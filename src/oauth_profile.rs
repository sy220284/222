//! Provider-neutral profile fetching, ported from
//! src/services/oauth/getOauthProfile.ts in 2.1.88.
use reqwest::{header, Client, Url};
use serde_json::Value;
use std::time::Duration;

/// API-specific profile paths remain configurable for non-vendor OAuth
/// endpoints; no account or API key is loaded from a proprietary store.
#[derive(Debug, Clone)]
pub struct ProfileEndpoints {
    pub base_url: Url,
    pub api_key_profile_path: String,
    pub oauth_profile_path: String,
    pub compatibility_header: Option<(String, String)>,
}

impl ProfileEndpoints {
    pub fn new(base_url: &str) -> anyhow::Result<Self> {
        let base_url = Url::parse(base_url)?;
        if !matches!(base_url.scheme(), "http" | "https") {
            anyhow::bail!("Profile endpoint must use HTTP(S)");
        }
        Ok(Self {
            base_url,
            api_key_profile_path: "/api/profile".into(),
            oauth_profile_path: "/api/oauth/profile".into(),
            compatibility_header: None,
        })
    }

    fn endpoint(&self, path: &str) -> Option<Url> {
        self.base_url.join(path).ok()
    }
}

fn client() -> Option<Client> {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()
}

/// Returns None for absent credentials, network errors, non-2xx responses
/// and invalid profile JSON, matching the optional source return value.
pub async fn get_oauth_profile_from_api_key(
    config: &ProfileEndpoints,
    account_uuid: Option<&str>,
    api_key: Option<&str>,
) -> Option<Value> {
    let account_uuid = account_uuid.filter(|uuid| !uuid.is_empty())?;
    let api_key = api_key.filter(|key| !key.is_empty())?;
    let url = config.endpoint(&config.api_key_profile_path)?;
    let mut request = client()?
        .get(url)
        .query(&[("account_uuid", account_uuid)])
        .header("x-api-key", api_key);
    if let Some((name, value)) = &config.compatibility_header {
        request = request.header(name.as_str(), value.as_str());
    }
    request.send().await.ok()?.error_for_status().ok()?.json().await.ok()
}

pub async fn get_oauth_profile_from_oauth_token(
    config: &ProfileEndpoints,
    access_token: &str,
) -> Option<Value> {
    if access_token.is_empty() {
        return None;
    }
    let url = config.endpoint(&config.oauth_profile_path)?;
    client()?
        .get(url)
        .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn fake_profile_server(body: &str, status: &str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let body = body.to_owned();
        let status = status.to_owned();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0u8; 1024];
                let count = stream.read(&mut buf).await.unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..count]);
                if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).await.unwrap();
            String::from_utf8(request).unwrap()
        });
        (base, server)
    }

    #[tokio::test]
    async fn user_profile_uses_account_uuid_api_key_and_compatible_headers() {
        let (base, server) = fake_profile_server(r#"{"account":{"id":"account-1"}}"#, "200 OK").await;
        let mut config = ProfileEndpoints::new(&base).unwrap();
        config.compatibility_header = Some(("x-provider-beta".into(), "profile-v1".into()));
        let profile = get_oauth_profile_from_api_key(
            &config,
            Some("user id"),
            Some("sample-api-key"),
        )
        .await
        .unwrap();
        assert_eq!(profile["account"]["id"], "account-1");
        let request = server.await.unwrap();
        assert!(request.starts_with("GET /api/profile?account_uuid=user+id "));
        assert!(request.contains("x-api-key: sample-api-key"));
        assert!(request.contains("x-provider-beta: profile-v1"));
        assert!(!request.contains("Authorization: Bearer"));
        assert_eq!(
            get_oauth_profile_from_api_key(&config, None, Some("key")).await,
            None
        );
    }

    #[tokio::test]
    async fn bearer_profile_uses_authorization_and_json_content_type() {
        let (base, server) = fake_profile_server(r#"{"user":{"name":"Test"}}"#, "200 OK").await;
        let config = ProfileEndpoints::new(&base).unwrap();
        let profile = get_oauth_profile_from_oauth_token(&config, "token-123")
            .await
            .unwrap();
        assert_eq!(profile["user"]["name"], "Test");
        let request = server.await.unwrap();
        assert!(request.starts_with("GET /api/oauth/profile "));
        assert!(request.contains("authorization: Bearer token-123"));
        assert!(request.contains("content-type: application/json"));
    }

    #[tokio::test]
    async fn http_error_does_not_fabricate_a_profile() {
        let (base, server) = fake_profile_server(r#"{"error":"invalid_token"}"#, "401 Unauthorized").await;
        let config = ProfileEndpoints::new(&base).unwrap();
        assert_eq!(
            get_oauth_profile_from_oauth_token(&config, "expired").await,
            None
        );
        server.await.unwrap();
    }
}
