//! Production remote SDK adapter: original 2.1.88 cli/remoteIO.ts.
//! Runs the existing SDK actor with private pipes and transports messages
//! through the selected live WebSocket, Hybrid or CCR/SSE protocol.
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::jwt_utils::TokenRefreshScheduler;
use crate::transports::ccr_client::CcrClient;
use crate::transports::hybrid::HybridTransport;
use crate::transports::sse::SseTransport;
use crate::transports::web_socket::{WebSocketTransport, WsState};
use crate::transports::{get_transport_for_url, sse_stream_url_for_session, TransportKind};

type Headers = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

enum RemoteChannel {
    WebSocket(Arc<WebSocketTransport>),
    Hybrid(Arc<HybridTransport>),
    Sse(Arc<SseTransport>, String),
    Ccr(Arc<CcrClient>),
}

impl RemoteChannel {
    async fn send(&self, event: &Value) -> Result<()> {
        match self {
            Self::WebSocket(socket) => {
                // WebSocket's offline buffer only retains UUID-bearing
                // messages. Wait for connection before sending SDK init.
                tokio::time::timeout(Duration::from_secs(30), async {
                    loop {
                        if socket.is_closed() {
                            bail!("Remote WebSocket closed before sending");
                        }
                        if socket.state() == WsState::Connected {
                            return socket.queue_message(event.clone()).await;
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                })
                .await
                .context("Remote WebSocket connection timeout")?
            }
            Self::Hybrid(hybrid) => {
                let before = hybrid.writes.dropped_batch_count();
                hybrid.writes.write(event).await;
                if hybrid.writes.dropped_batch_count() != before {
                    bail!("Hybrid transport dropped an outbound batch");
                }
                Ok(())
            }
            Self::Sse(stream, url) => stream.write(url, event).await,
            Self::Ccr(ccr) => {
                let before = ccr.dropped_event_batch_count();
                ccr.write_event(event).await;
                ccr.flush().await;
                if ccr.dropped_event_batch_count() != before {
                    bail!("CCR worker dropped an outbound event batch");
                }
                Ok(())
            }
        }
    }

    async fn close(&self) {
        match self {
            Self::WebSocket(ws) => ws.close(),
            Self::Hybrid(hybrid) => hybrid.close().await,
            Self::Sse(sse, _) => sse.close(),
            Self::Ccr(ccr) => ccr.close(),
        }
    }

    fn delivery(&self, id: &str, status: &str) {
        if let Self::Ccr(ccr) = self {
            ccr.report_delivery(id, status);
        }
    }
}

fn validate_url(url: &str, token_present: bool) -> Result<()> {
    let parsed = reqwest::Url::parse(url).context("Invalid remote SDK URL")?;
    if !matches!(parsed.scheme(), "ws" | "wss" | "http" | "https") {
        bail!("Remote SDK requires a WebSocket or HTTP(S) endpoint");
    }
    let host = parsed.host_str().context("Remote SDK URL has no host")?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if token_present && !matches!(parsed.scheme(), "wss" | "https") && !loopback {
        bail!("Remote credentials require TLS except for loopback connections");
    }
    Ok(())
}

fn read_token(file: Option<&Path>) -> Result<Option<String>> {
    file.map(|path| {
        let value = std::fs::read_to_string(path)
            .with_context(|| format!("Unable to read credential file {}", path.display()))?;
        let value = value.trim().to_owned();
        if value.is_empty() {
            bail!("Remote credential file is empty");
        }
        Ok(value)
    })
    .transpose()
}

fn start_sdk(cli: &crate::Cli, session_id: &str, resume: Option<&str>) -> Result<Child> {
    use std::process::Stdio;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("--workspace").arg(&cli.workspace)
        .arg("--max-steps").arg(cli.max_steps.to_string())
        .arg("--approval").arg(match cli.approval {
            crate::ApprovalArg::ReadOnly => "read-only",
            crate::ApprovalArg::Ask => "ask",
            crate::ApprovalArg::Full => "full",
        });
    if let Some(provider) = &cli.provider {
        cmd.arg("--provider").arg(provider);
    }
    if let Some(model) = &cli.model {
        cmd.arg("--model").arg(model);
    }
    for directory in &cli.additional_directory {
        cmd.arg("--add-dir").arg(directory);
    }
    cmd.arg("sdk");
    if let Some(id) = resume {
        cmd.arg("--resume").arg(id);
    } else {
        cmd.arg("--remote-session-id").arg(session_id);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    cmd.spawn().context("Unable to start SDK child")
}

async fn deliver_input(
    channel: &RemoteChannel,
    event: &Value,
    stdin: &mut tokio::process::ChildStdin,
    pending: &mut std::collections::HashMap<String, String>,
) -> Result<()> {
    let id = event.get("eventId").or_else(|| event.get("event_id"))
        .and_then(Value::as_str);
    if let (Some(id), Some(uuid)) = (id, event.get("uuid").and_then(Value::as_str)) {
        pending.insert(uuid.to_owned(), id.to_owned());
        channel.delivery(id, "received");
    }
    stdin.write_all(crate::ndjson::stringify_value(event).as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    if let Some(id) = id {
        channel.delivery(id, "processing");
    }
    Ok(())
}

/// Start an authenticated remote session. The local JSON-line SDK actor uses
/// private stdio; remote delivery succeeds or fails independently of stdout.
pub async fn run(
    cli: &crate::Cli,
    url: &str,
    session_id: &str,
    resume: Option<&str>,
    token_file: Option<&Path>,
) -> Result<()> {
    let original_token = read_token(token_file)?;
    validate_url(url, original_token.is_some())?;
    let token = Arc::new(RwLock::new(original_token.clone()));
    let headers: Headers = {
        let token = Arc::clone(&token);
        Arc::new(move || token.read().expect("Remote token lock poisoned")
            .as_deref()
            .map(|value| vec![("Authorization".into(), format!("Bearer {value}"))])
            .unwrap_or_default())
    };
    let refresh = token_file.map(|path| {
        let path = path.to_owned();
        let token = Arc::clone(&token);
        let owner = session_id.to_owned();
        TokenRefreshScheduler::new(
            move || {
                let path = path.clone();
                async move {
                    let next = tokio::fs::read_to_string(path).await?;
                    Ok(Some(next.trim().to_owned()).filter(|s| !s.is_empty()))
                }
            },
            move |id, value| {
                if id == owner {
                    *token.write().expect("Remote token lock poisoned") = Some(value.to_owned());
                }
            },
        )
    });
    if let (Some(refresh), Some(token)) = (&refresh, &original_token) {
        refresh.schedule(session_id, token);
    }

    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<String>();
    let (incoming_tx, mut incoming_rx) = mpsc::channel::<Value>(1024);
    let ws_input_tx = incoming_tx.clone();
    let ws_forwarder = tokio::spawn(async move {
        while let Some(frame) = ws_rx.recv().await {
            for line in frame.lines().filter(|s| !s.trim().is_empty()) {
                match serde_json::from_str::<Value>(line) {
                    Ok(value) => {
                        if ws_input_tx.send(value).await.is_err() {
                            return;
                        }
                    }
                    Err(error) => eprintln!("Remote SDK received invalid JSON: {error}"),
                }
            }
        }
    });

    let (channel, mut driver, sse_forwarder) = match get_transport_for_url(url)? {
        TransportKind::WebSocket => {
            let ws = Arc::new(WebSocketTransport::new(
                url.to_owned(), Vec::new(), Some(session_id.to_owned()),
                ws_tx, true, Some(Arc::clone(&headers)),
            ));
            let reader = Arc::clone(&ws);
            let driver = tokio::spawn(async move {
                reader.run().await;
                bail!("Remote WebSocket ingress terminated")
            });
            (RemoteChannel::WebSocket(ws), driver, None)
        }
        TransportKind::Hybrid => {
            let hybrid = Arc::new(HybridTransport::new(
                url.to_owned(), Vec::new(), Some(session_id.to_owned()),
                ws_tx, Arc::clone(&headers),
            )?);
            let reader = Arc::clone(&hybrid);
            let driver = tokio::spawn(async move {
                reader.reads.run().await;
                bail!("Remote Hybrid ingress terminated")
            });
            (RemoteChannel::Hybrid(hybrid), driver, None)
        }
        TransportKind::Sse => {
            let stream_url = sse_stream_url_for_session(url)?;
            let sse = Arc::new(SseTransport::new(
                stream_url.clone(), Vec::new(), Some(session_id.to_owned()),
                Arc::clone(&headers),
            ));
            let receiver = sse.event_rx.lock().expect("SSE receiver lock poisoned")
                .take().context("SSE event receiver already taken")?;
            let sse_sender = incoming_tx.clone();
            let forwarder = tokio::spawn(async move {
                let mut receiver = receiver;
                while let Some(event) = receiver.recv().await {
                    if sse_sender.send(event).await.is_err() {
                        return;
                    }
                }
            });
            let channel = if std::env::var("ATLAS_USE_CCR_V2").ok()
                .is_some_and(|flag| matches!(flag.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
            {
                let worker_token = original_token.as_deref()
                    .context("CCR v2 requires a worker token file")?;
                let epoch = crate::work_secret::register_worker(url, worker_token).await?;
                let mut ccr = CcrClient::new(&sse, url, Arc::clone(&headers), None, None, None);
                ccr.initialize(session_id, Some(epoch)).await?;
                RemoteChannel::Ccr(Arc::new(ccr))
            } else {
                RemoteChannel::Sse(
                    Arc::clone(&sse),
                    crate::transports::convert_sse_url_to_post_url(&stream_url)?,
                )
            };
            let reader = Arc::clone(&sse);
            let driver = tokio::spawn(async move { reader.run().await });
            (channel, driver, Some(forwarder))
        }
    };

    let mut child = start_sdk(cli, session_id, resume)?;
    let mut stdin = child.stdin.take().context("SDK child missing stdin")?;
    let stdout = child.stdout.take().context("SDK child missing stdout")?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut pending = std::collections::HashMap::<String, String>::new();
    let outcome: Result<()> = async {
        loop {
            tokio::select! {
                completed = &mut driver => {
                    return Err(match completed {
                        Ok(Err(error)) => error,
                        Ok(Ok(())) => anyhow!("Remote ingress closed"),
                        Err(error) => anyhow!("Remote ingress task failed: {error}"),
                    });
                }
                next = incoming_rx.recv() => {
                    let Some(event) = next else { bail!("Remote input channel ended"); };
                    deliver_input(&channel, &event, &mut stdin, &mut pending).await?;
                }
                next = stdout.next_line() => {
                    let Some(line) = next.context("Unable to read SDK child output")? else {
                        let status = child.wait().await?;
                        if status.success() {
                            return Ok(());
                        }
                        bail!("Local SDK process exited with {status}");
                    };
                    let event: Value = serde_json::from_str(&line)
                        .context("SDK child emitted malformed JSON")?;
                    channel.send(&event).await?;
                    if let Some(uuid) = event.get("uuid").and_then(Value::as_str) {
                        if let Some(event_id) = pending.remove(uuid) {
                            channel.delivery(&event_id, "processed");
                        }
                    }
                }
            }
        }
    }.await;
    if let Some(scheduler) = &refresh {
        scheduler.cancel(session_id);
    }
    channel.close().await;
    driver.abort();
    ws_forwarder.abort();
    if let Some(forwarder) = sse_forwarder {
        forwarder.abort();
    }
    // Terminate the SDK actor if the transport fails during a model call.
    let _ = child.start_kill();
    let _ = child.wait().await;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_credential_transport_and_loopback() {
        assert!(validate_url("wss://example.com/s1", true).is_ok());
        assert!(validate_url("ws://example.com/s1", true).is_err());
        assert!(validate_url("ws://127.0.0.1:9000/s1", true).is_ok());
        assert!(validate_url("ws://[::1]:9000/s1", true).is_ok());
        assert!(validate_url("ftp://example.com/s1", false).is_err());
    }
}
