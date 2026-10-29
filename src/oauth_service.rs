//! OAuth authorization flow, manual completion and local credential
//! persistence. Port of src/services/oauth/index.ts (2.1.88).
use crate::{
    oauth_callback_listener::AuthCodeListener,
    oauth_client::{AuthorizationLinks, AuthorizationOptions, OAuthProvider, OAuthTokens},
    oauth_crypto::PkceRequest,
};
use anyhow::{bail, Context, Result};
use std::{
    fs::{self, OpenOptions},
    future::Future,
    io::Write,
    path::Path,
};
use uuid::Uuid;

pub struct OAuthService {
    pub pkce: PkceRequest,
}

impl OAuthService {
    pub fn new() -> Result<Self> {
        Ok(Self {
            pkce: PkceRequest::create()?,
        })
    }

    pub fn manual_authorization_url(
        &self,
        provider: &OAuthProvider,
        options: &AuthorizationOptions,
    ) -> String {
        provider
            .build_auth_url(
                &self.pkce.challenge,
                &self.pkce.state,
                &provider.manual_redirect_uri,
                options,
            )
            .to_string()
    }

    pub async fn finish_manual(
        &self,
        provider: &OAuthProvider,
        authorization_code: &str,
    ) -> Result<OAuthTokens> {
        provider
            .exchange_code_for_tokens(
                authorization_code,
                &self.pkce.state,
                &self.pkce.verifier,
                &provider.manual_redirect_uri,
                None,
            )
            .await
    }

    /// Open local callback and publish both authorization URLs. The caller
    /// controls presentation/opening, as in the source SDK's skipBrowserOpen
    /// path. Browser success occurs only after a successful token exchange.
    pub async fn run_automatic<F, Fut>(
        &self,
        provider: &OAuthProvider,
        options: &AuthorizationOptions,
        success_redirect_url: &str,
        error_redirect_url: &str,
        publish: F,
    ) -> Result<OAuthTokens>
    where
        F: FnOnce(AuthorizationLinks) -> Fut,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let mut listener = AuthCodeListener::start(&provider.callback_path, None).await?;
        let links = provider.authorization_links(&self.pkce, listener.port(), options);
        let state = &self.pkce.state;
        let authorization_code = listener
            .wait_for_authorization(state, || publish(links))
            .await;
        let code = match authorization_code {
            Ok(code) => code,
            Err(error) => {
                listener.close().await?;
                return Err(error);
            }
        };
        let automatic = listener.has_pending_response();
        let result = provider
            .exchange_code_for_tokens(
                &code,
                state,
                &self.pkce.verifier,
                &provider.automatic_redirect_uri(listener.port()),
                None,
            )
            .await;
        if automatic {
            let redirect = match &result {
                Ok(_) => listener.handle_success_redirect(success_redirect_url).await,
                Err(_) => listener.handle_error_redirect(error_redirect_url).await,
            };
            listener.close().await?;
            redirect?;
        } else {
            listener.close().await?;
        }
        result
    }
}

/// Explicitly user-selected credential file; never store in the source tree
/// by default. Create with restrictive permissions and rename atomically.
pub fn save_oauth_tokens(path: &Path, tokens: &OAuthTokens) -> Result<()> {
    let parent = path.parent().context("Credential path needs parent directory")?;
    fs::create_dir_all(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            bail!("Credential path is a symbolic link");
        }
    }
    let temporary = parent.join(format!(".oauth-{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    let write_result = (|| -> Result<()> {
        serde_json::to_writer(&mut file, tokens)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    write_result
}

pub fn load_oauth_tokens(path: &Path) -> Result<OAuthTokens> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("OAuth credentials must be an ordinary file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            bail!("OAuth credentials must not be accessible by group or other");
        }
    }
    serde_json::from_slice(&fs::read(path)?).context("OAuth credential file is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

    #[tokio::test]
    async fn manual_authorization_preserves_verifier_and_redirect_uri() {
        let server = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://127.0.0.1:{}", server.local_addr().unwrap().port());
        let provider = OAuthProvider::new(
            &format!("{base}/authorize"),
            &format!("{base}/token"),
            "client-123",
            vec!["openid".into(), "profile".into()],
            "https://example.org/manual",
        ).unwrap();
        let flow = OAuthService::new().unwrap();
        let auth = flow.manual_authorization_url(&provider, &AuthorizationOptions::default());
        let url = reqwest::Url::parse(&auth).unwrap();
        let values: std::collections::HashMap<_,_> = url.query_pairs().into_owned().collect();
        assert_eq!(values["state"], flow.pkce.state);
        assert_eq!(values["code_challenge"], flow.pkce.challenge);
        assert_eq!(values["redirect_uri"], "https://example.org/manual");
        let expected_verifier = flow.pkce.verifier.clone();
        let handler = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let mut data = Vec::new();
            let mut bytes = [0u8; 1024];
            loop {
                let count = stream.read(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                data.extend_from_slice(&bytes[..count]);
                if data.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&data);
            assert!(request.contains(&expected_verifier));
            assert!(request.contains("manual"));
            let response = r#"{"access_token":"test-access","refresh_token":"test-refresh","expires_in":600,"scope":"openid profile"}"#;
            stream.write_all(format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(), response
            ).as_bytes()).await.unwrap();
        });
        let tokens = flow.finish_manual(&provider, "auth-123").await.unwrap();
        assert_eq!(tokens.access_token, "test-access");
        handler.await.unwrap();
    }

    #[test]
    fn tokens_roundtrip_to_private_file_and_reject_weak_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("credentials.json");
        let record = OAuthTokens {
            access_token: "secret-access".into(),
            refresh_token: "secret-refresh".into(),
            expires_at_ms: 100_000,
            scopes: vec!["openid".into()],
            account: None,
            organization: None,
        };
        save_oauth_tokens(&path, &record).unwrap();
        let read = load_oauth_tokens(&path).unwrap();
        assert_eq!(read.refresh_token, "secret-refresh");
        assert_eq!(read.scopes, ["openid"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(load_oauth_tokens(&path).is_err());
        }
    }
}
