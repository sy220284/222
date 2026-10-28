use serde_json::json;
use std::{fs, io::{Read, Write}, net::{TcpListener, TcpStream}, process::Command, thread, time::Duration};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

#[test]
fn cli_initialization_and_model_listing_work_offline() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    let init = binary().args(["init"]).env("ATLAS_CONFIG", &config).output().unwrap();
    assert!(init.status.success(), "{:?}", init);
    assert!(config.is_file());
    let list = binary().args(["models"]).env("ATLAS_CONFIG", &config).output().unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8(list.stdout).unwrap();
    assert!(stdout.contains("local (default)"));
    assert!(stdout.contains("gemini"));
}

fn read_request(stream: &mut TcpStream) -> String {
    stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let mut data = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 { break; }
        data.extend_from_slice(&chunk[..count]);
        if let Some(index) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&data[..index]);
            let length = headers.lines().find_map(|line| {
                line.to_ascii_lowercase().strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            }).unwrap_or(0);
            if data.len() >= index + 4 + length { break; }
        }
    }
    String::from_utf8(data).unwrap()
}

#[test]
fn real_http_tool_roundtrip_and_session_persistence() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("example.txt"), "hello from workspace\n").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            requests.push(request);
            let payload = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[
                    {"id":"call_1","type":"function","function":{
                        "name":"read_file","arguments":"{\"path\":\"example.txt\"}"
                    }}
                ]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant","content":"Read verified."},
                    "finish_reason":"stop"}]})
            };
            let body = payload.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });

    let config = dir.path().join("test-config.json");
    fs::write(&config, json!({
        "default_provider":"mock",
        "providers":[{"id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"test-model"}]
    }).to_string()).unwrap();
    let output = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "run", "Inspect example.txt"])
        .env("ATLAS_CONFIG", &config)
        .output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Read verified."));
    assert!(requests[0].contains("Inspect example.txt"));
    assert!(requests[1].contains("hello from workspace"));
    let session_dir = dir.path().join(".atlas/sessions");
    let sessions: Vec<_> = fs::read_dir(session_dir).unwrap().collect();
    assert_eq!(sessions.len(), 1);
    let saved = fs::read_to_string(sessions[0].as_ref().unwrap().path()).unwrap();
    assert!(saved.contains("call_1"));
    assert!(saved.contains("Read verified."));
}

#[test]
fn gemini_native_adapter_completes_tool_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("example.txt"), "gemini test file\n").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut received = Vec::new();
        for i in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            received.push(read_request(&mut stream));
            let payload = if i == 0 {
                json!({"candidates":[{"content":{"parts":[
                    {"functionCall":{"name":"read_file","args":{"path":"example.txt"}}}
                ]},"finishReason":"STOP"}]})
            } else {
                json!({"candidates":[{"content":{"parts":[{"text":"Gemini read verified."}]},"finishReason":"STOP"}]})
            };
            let body = payload.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        received
    });
    let config = dir.path().join("gemini-config.json");
    fs::write(&config, json!({
        "default_provider":"gm",
        "providers":[{"id":"gm","kind":"gemini",
            "base_url":format!("http://{address}/v1beta"),
            "model":"example","api_key_env":"ATLAS_TEST_GEMINI_KEY"}]
    }).to_string()).unwrap();
    let output = binary().args([
        "--workspace",dir.path().to_str().unwrap(),"run","Read example.txt"
    ]).env("ATLAS_CONFIG", &config).env("ATLAS_TEST_GEMINI_KEY","fake-local-key")
        .output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Gemini read verified."));
    assert!(requests[0].contains("functionDeclarations"));
    assert!(requests[1].contains("functionResponse"));
    assert!(requests[1].contains("gemini test file"));
}
