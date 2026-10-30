//! Real local WebSocket server ↔ production remote SDK ↔ SDK child process.
//! Proves that init and control messages traverse the complete live chain.
use base64::Engine;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn read_headers(peer: &mut TcpStream) -> String {
    let mut data = Vec::new();
    let mut chunk = [0u8; 2048];
    while !data.windows(4).any(|part| part == b"\r\n\r\n") {
        let count = peer.read(&mut chunk).unwrap();
        assert!(count > 0 && data.len() < 16_384);
        data.extend_from_slice(&chunk[..count]);
    }
    String::from_utf8(data).unwrap()
}

fn ws_accept(key: &str) -> String {
    let source = format!("{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11", key.trim());
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, source.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(digest.as_ref())
}

fn send_json(peer: &mut TcpStream, event: Value) {
    let payload = format!("{event}\n");
    let bytes = payload.as_bytes();
    let mut frame = vec![0x81];
    if bytes.len() < 126 {
        frame.push(bytes.len() as u8);
    } else {
        frame.push(126);
        frame.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(bytes);
    peer.write_all(&frame).unwrap();
}

fn read_json(peer: &mut TcpStream) -> Value {
    loop {
        let mut head = [0u8; 2];
        peer.read_exact(&mut head).unwrap();
        assert_eq!(head[1] & 0x80, 0x80, "client frames must be masked");
        let mut len = u64::from(head[1] & 0x7f);
        if len == 126 {
            let mut longer = [0u8; 2];
            peer.read_exact(&mut longer).unwrap();
            len = u64::from(u16::from_be_bytes(longer));
        } else if len == 127 {
            let mut longer = [0u8; 8];
            peer.read_exact(&mut longer).unwrap();
            len = u64::from_be_bytes(longer);
        }
        assert!(len <= 16 * 1024 * 1024);
        let mut mask = [0u8; 4];
        peer.read_exact(&mut mask).unwrap();
        let mut payload = vec![0u8; len as usize];
        peer.read_exact(&mut payload).unwrap();
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
        if head[0] & 0xf == 0x9 {
            let mut pong = vec![0x8a, payload.len() as u8];
            pong.extend_from_slice(&payload);
            peer.write_all(&pong).unwrap();
            continue;
        }
        assert_eq!(head[0] & 0xf, 0x1);
        return serde_json::from_slice(&payload).unwrap();
    }
}

fn accept_with_deadline(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        match listener.accept() {
            Ok((peer, _)) => {
                peer.set_nonblocking(false).unwrap();
                peer.set_read_timeout(Some(Duration::from_secs(15)))
                    .unwrap();
                peer.set_write_timeout(Some(Duration::from_secs(15)))
                    .unwrap();
                return peer;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "remote SDK did not connect");
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("accept WebSocket: {e}"),
        }
    }
}

#[test]
fn remote_sdk_process_handles_initialize_and_shutdown_over_real_websocket() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "model":"test-model","base_url":"http://127.0.0.1:1234/v1"}]
        })
        .to_string(),
    )
    .unwrap();
    let token = root.path().join("access-token");
    fs::write(&token, "test-local-token\n").unwrap();
    let session = uuid::Uuid::new_v4();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let mut peer = accept_with_deadline(&listener);
        let headers = read_headers(&mut peer);
        assert!(headers.contains("Authorization: Bearer test-local-token"));
        assert!(headers.contains(&format!("X-Session-Id: {session}")));
        let key = headers
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap();
        let reply = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            ws_accept(key),
        );
        peer.write_all(reply.as_bytes()).unwrap();
        let initial = read_json(&mut peer);
        assert_eq!(initial["type"], "system");
        assert_eq!(initial["session_id"], session.to_string());
        send_json(
            &mut peer,
            json!({
                "type":"control_request","request_id":"remote-init",
                "request":{"subtype":"initialize"}
            }),
        );
        let response = read_json(&mut peer);
        assert_eq!(response["type"], "control_response");
        assert_eq!(response["response"]["request_id"], "remote-init");
        assert_eq!(response["response"]["subtype"], "success");

        send_json(
            &mut peer,
            json!({
                "type":"control_request","request_id":"remote-stop",
                "request":{"subtype":"end_session"}
            }),
        );
        let end = read_json(&mut peer);
        assert_eq!(end["type"], "control_response");
        assert_eq!(end["response"]["request_id"], "remote-stop");
        thread::sleep(Duration::from_millis(200));
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "sdk",
            "--remote-url",
            &format!("ws://127.0.0.1:{port}/session/{session}"),
            "--remote-session-id",
            &session.to_string(),
            "--token-file",
            token.to_str().unwrap(),
        ])
        .env("ATLAS_CONFIG", &config)
        .env_remove("ATLAS_USE_CCR_V2")
        .env_remove("ATLAS_POST_FOR_SESSION_INGRESS_V2")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("remote SDK failed to close after end_session");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "remote mode must not echo all SDK messages to stdout"
    );
}


/// Real Hybrid ingress (WebSocket) and egress (serialized HTTP POST) must
/// preserve SDK initialization and control-response order.
#[test]
fn remote_sdk_hybrid_delivers_real_post_bodies_in_order() {
    fn read_post(peer: &mut TcpStream) -> Value {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 4096];
        let boundary = loop {
            if let Some(index) = buffer.windows(4).position(|part| part == b"\r\n\r\n") {
                break index + 4;
            }
            let n = peer.read(&mut chunk).unwrap();
            assert!(n > 0 && buffer.len() < 65_536);
            buffer.extend_from_slice(&chunk[..n]);
        };
        let headers = String::from_utf8_lossy(&buffer[..boundary]);
        assert!(headers.starts_with("POST /v2/session_ingress/session/"));
        assert!(headers.contains("Authorization: Bearer test-local-token"));
        let length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
            })
            .expect("Hybrid POST requires Content-Length");
        while buffer.len() - boundary < length {
            let n = peer.read(&mut chunk).unwrap();
            assert!(n > 0);
            buffer.extend_from_slice(&chunk[..n]);
        }
        let body = serde_json::from_slice(&buffer[boundary..boundary + length]).unwrap();
        peer.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
        body
    }

    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "model":"test-model","base_url":"http://127.0.0.1:1234/v1"}]
        })
        .to_string(),
    )
    .unwrap();
    let token = root.path().join("access-token");
    fs::write(&token, "test-local-token\n").unwrap();
    let session = uuid::Uuid::new_v4();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let mut websocket = accept_with_deadline(&listener);
        let headers = read_headers(&mut websocket);
        assert!(headers.contains("Authorization: Bearer test-local-token"));
        let key = headers
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap();
        let reply = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            ws_accept(key),
        );
        websocket.write_all(reply.as_bytes()).unwrap();
        let mut first = accept_with_deadline(&listener);
        let body = read_post(&mut first);
        assert_eq!(body["events"][0]["type"], "system");
        assert_eq!(body["events"][0]["session_id"], session.to_string());
        send_json(
            &mut websocket,
            json!({
                "type":"control_request","request_id":"hybrid-init",
                "request":{"subtype":"initialize"}
            }),
        );
        let mut second = accept_with_deadline(&listener);
        let reply = read_post(&mut second);
        assert_eq!(reply["events"][0]["type"], "control_response");
        assert_eq!(reply["events"][0]["response"]["request_id"], "hybrid-init");
        assert_eq!(reply["events"][0]["response"]["subtype"], "success");
        send_json(
            &mut websocket,
            json!({
                "type":"control_request","request_id":"hybrid-stop",
                "request":{"subtype":"end_session"}
            }),
        );
        let mut third = accept_with_deadline(&listener);
        let end = read_post(&mut third);
        assert_eq!(end["events"][0]["response"]["request_id"], "hybrid-stop");
        thread::sleep(Duration::from_millis(200));
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "sdk",
            "--remote-url",
            &format!("ws://127.0.0.1:{port}/v2/session_ingress/ws/{session}"),
            "--remote-session-id",
            &session.to_string(),
            "--token-file",
            token.to_str().unwrap(),
        ])
        .env("ATLAS_CONFIG", &config)
        .env_remove("ATLAS_USE_CCR_V2")
        .env("ATLAS_POST_FOR_SESSION_INGRESS_V2", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("Hybrid SDK failed to exit after remote end_session");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
}
