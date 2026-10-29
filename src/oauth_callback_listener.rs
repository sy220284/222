//! OAuth authorization-code callback listener, ported from
//! src/services/oauth/auth-code-listener.ts (2.1.88).
//! Binds only to the local loopback interface and never sends telemetry.
use anyhow::{bail, Context, Result};
use std::{future::Future, net::IpAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

const MAX_REQUEST_HEADERS: usize = 16 * 1024;

pub struct AuthCodeListener {
    server: Option<TcpListener>,
    pending_response: Option<TcpStream>,
    callback_path: String,
    port: u16,
}

impl AuthCodeListener {
    pub async fn start(callback_path: &str, port: Option<u16>) -> Result<Self> {
        if !callback_path.starts_with('/')
            || callback_path.starts_with("//")
            || callback_path.contains('?')
            || callback_path.contains('#')
        {
            bail!("OAuth callback path must be an absolute URL path");
        }
        let server = TcpListener::bind(("127.0.0.1", port.unwrap_or(0)))
            .await
            .context("Failed to start OAuth callback server")?;
        let actual_port = server.local_addr()?.port();
        Ok(Self {
            server: Some(server),
            pending_response: None,
            callback_path: callback_path.to_owned(),
            port: actual_port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn has_pending_response(&self) -> bool {
        self.pending_response.is_some()
    }

    /// The redirect response stays open until the caller finishes exchanging
    /// the code, preserving the original browser completion behavior.
    pub async fn wait_for_authorization<F, Fut>(
        &mut self,
        expected_state: &str,
        on_ready: F,
    ) -> Result<String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        on_ready().await?;
        loop {
            let (mut stream, address) = self
                .server
                .as_ref()
                .context("OAuth callback listener is closed")?
                .accept()
                .await?;
            if !matches!(address.ip(), IpAddr::V4(ip) if ip.is_loopback()) {
                send_response(&mut stream, 403, "Forbidden", None).await?;
                continue;
            }
            // Node's HTTP server rejects malformed client requests without
            // cancelling the outstanding authorization promise. Keep the
            // loopback listener alive for the real browser callback.
            let request = match read_headers(&mut stream).await {
                Ok(request) => request,
                Err(HeaderReadError::TooLarge) => {
                    let _ = send_response(&mut stream, 431, "Request Header Fields Too Large", None).await;
                    continue;
                }
                Err(HeaderReadError::Timeout) => {
                    let _ = send_response(&mut stream, 408, "Request Timeout", None).await;
                    continue;
                }
                Err(HeaderReadError::Invalid) => {
                    let _ = send_response(&mut stream, 400, "Bad Request", None).await;
                    continue;
                }
            };
            let Some(first_line) = request.lines().next() else {
                send_response(&mut stream, 400, "Bad Request", None).await?;
                continue;
            };
            let mut words = first_line.split_whitespace();
            let method = words.next().unwrap_or("");
            let target = words.next().unwrap_or("");
            if method != "GET" || !target.starts_with('/') {
                send_response(&mut stream, 400, "Bad Request", None).await?;
                continue;
            }
            let parsed = match reqwest::Url::parse(&format!("http://localhost{target}")) {
                Ok(parsed) => parsed,
                Err(_) => {
                    let _ = send_response(&mut stream, 400, "Bad Request", None).await;
                    continue;
                }
            };
            if parsed.path() != self.callback_path {
                send_response(&mut stream, 404, "Not Found", None).await?;
                continue;
            }
            let mut code = None;
            let mut state = None;
            for (key, value) in parsed.query_pairs() {
                if key == "code" && code.is_none() {
                    code = Some(value.into_owned());
                } else if key == "state" && state.is_none() {
                    state = Some(value.into_owned());
                }
            }
            let code = match code.filter(|code| !code.is_empty()) {
                Some(code) => code,
                None => {
                    send_response(&mut stream, 400, "Authorization code not found", None).await?;
                    bail!("No authorization code received");
                }
            };
            if state.as_deref() != Some(expected_state) {
                send_response(&mut stream, 400, "Invalid state parameter", None).await?;
                bail!("Invalid state parameter");
            }
            self.pending_response = Some(stream);
            return Ok(code);
        }
    }

    /// This is deliberately configurable by the OAuth provider; there are
    /// no vendor-specific success URLs or account-type restrictions.
    pub async fn handle_success_redirect(&mut self, success_url: &str) -> Result<()> {
        self.redirect_pending(success_url).await
    }

    pub async fn handle_error_redirect(&mut self, error_url: &str) -> Result<()> {
        self.redirect_pending(error_url).await
    }

    async fn redirect_pending(&mut self, target: &str) -> Result<()> {
        let url = reqwest::Url::parse(target)?;
        if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() {
            bail!("OAuth completion redirect requires a valid HTTP(S) URL");
        }
        if let Some(mut stream) = self.pending_response.take() {
            send_response(&mut stream, 302, "Found", Some(url.as_str())).await?;
        }
        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut stream) = self.pending_response.take() {
            send_response(&mut stream, 400, "Authorization failed", None).await?;
        }
        self.server.take();
        Ok(())
    }
}

#[derive(Debug)]
enum HeaderReadError {
    Invalid,
    TooLarge,
    Timeout,
}

async fn read_headers(stream: &mut TcpStream) -> std::result::Result<String, HeaderReadError> {
    let read = async {
        let mut data = Vec::with_capacity(1024);
        loop {
            let mut block = [0u8; 1024];
            let count = stream
                .read(&mut block)
                .await
                .map_err(|_| HeaderReadError::Invalid)?;
            if count == 0 {
                return Err(HeaderReadError::Invalid);
            }
            data.extend_from_slice(&block[..count]);
            if data.len() > MAX_REQUEST_HEADERS {
                return Err(HeaderReadError::TooLarge);
            }
            if data.windows(4).any(|slice| slice == b"\\r\\n\\r\\n") {
                return String::from_utf8(data).map_err(|_| HeaderReadError::Invalid);
            }
        }
    };
    timeout(Duration::from_secs(15), read)
        .await
        .map_err(|_| HeaderReadError::Timeout)?
}

async fn send_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
    redirect: Option<&str>,
) -> Result<()> {
    let location = redirect
        .map(|url| format!("Location: {url}\r\n"))
        .unwrap_or_default();
    let payload = if redirect.is_some() { "" } else { body };
    let response = format!(
        "HTTP/1.1 {status} {body}\r\n{location}Content-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn request(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        String::from_utf8(response).unwrap()
    }

    async fn raw_request(port: u16, bytes: &[u8]) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(bytes).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        String::from_utf8(response).unwrap()
    }

    #[tokio::test]
    async fn malformed_http_and_oversized_headers_do_not_abort_browser_authorization() {
        let mut listener = AuthCodeListener::start("/callback", None).await.unwrap();
        let port = listener.port();
        let server = tokio::spawn(async move {
            let code = listener
                .wait_for_authorization("expected", || async { Ok(()) })
                .await
                .unwrap();
            assert_eq!(code, "real-code");
            listener
                .handle_success_redirect("https://example.test/complete")
                .await
                .unwrap();
            listener.close().await.unwrap();
        });

        let malformed = raw_request(
            port,
            b"GET /callback HTTP/1.1\\r\\nHost: localhost\\r\\nX-Invalid: \\xff\\r\\n\\r\\n",
        )
        .await;
        assert!(malformed.starts_with("HTTP/1.1 400"), "{malformed}");

        let large = format!(
            "GET /callback HTTP/1.1\\r\\nHost: localhost\\r\\nX-Large: {}\\r\\n\\r\\n",
            "x".repeat(MAX_REQUEST_HEADERS)
        );
        let oversized = raw_request(port, large.as_bytes()).await;
        assert!(oversized.starts_with("HTTP/1.1 431"), "{oversized}");

        let valid = request(port, "/callback?code=real-code&state=expected").await;
        assert!(valid.starts_with("HTTP/1.1 302"), "{valid}");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn valid_state_yields_code_then_redirects_browser() {
        let mut listener = AuthCodeListener::start("/callback", None).await.unwrap();
        let port = listener.port();
        assert_ne!(port, 0);
        let server = tokio::spawn(async move {
            let code = listener
                .wait_for_authorization("expected", || async { Ok(()) })
                .await
                .unwrap();
            assert_eq!(code, "A B");
            assert!(listener.has_pending_response());
            listener
                .handle_success_redirect("https://example.test/complete")
                .await
                .unwrap();
            assert!(!listener.has_pending_response());
            listener.close().await.unwrap();
        });
        let wrong = request(port, "/not-the-callback?code=ignore&state=expected").await;
        assert!(wrong.starts_with("HTTP/1.1 404"));
        let valid = request(port, "/callback?code=A+B&state=expected").await;
        assert!(valid.starts_with("HTTP/1.1 302"));
        assert!(valid.contains("Location: https://example.test/complete"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_missing_code_and_csrf_state_without_leaking_code() {
        for (request_path, body) in [
            ("/callback?state=expected", "Authorization code not found"),
            (
                "/callback?code=secret&state=other",
                "Invalid state parameter",
            ),
        ] {
            let mut listener = AuthCodeListener::start("/callback", None).await.unwrap();
            let port = listener.port();
            let server = tokio::spawn(async move {
                let outcome = listener
                    .wait_for_authorization("expected", || async { Ok(()) })
                    .await;
                assert!(outcome.is_err());
                assert!(!listener.has_pending_response());
                listener.close().await.unwrap();
            });
            let response = request(port, request_path).await;
            assert!(response.starts_with("HTTP/1.1 400"));
            assert!(response.contains(body));
            assert!(!response.contains("secret"));
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn on_ready_failure_does_not_accept_authorization_requests() {
        let mut listener = AuthCodeListener::start("/callback", None).await.unwrap();
        let result = listener
            .wait_for_authorization("expected", || async { bail!("Browser launch failed") })
            .await;
        assert!(result.is_err());
        assert!(!listener.has_pending_response());
        listener.close().await.unwrap();
    }
}
