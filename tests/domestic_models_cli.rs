//! Offline transport checks for bundled API model IDs and legacy settings.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn read_request(stream: &mut TcpStream) -> (String, Value) {
    stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let header_end = loop {
        let count = stream.read(&mut buffer).expect("read headers");
        assert!(count > 0, "HTTP request ended before headers");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .expect("Content-Length");
    while bytes.len() < header_end + length {
        let count = stream.read(&mut buffer).expect("read body");
        assert!(count > 0, "HTTP request body ended early");
        bytes.extend_from_slice(&buffer[..count]);
    }
    (
        headers,
        serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap(),
    )
}

#[test]
fn old_config_lists_domestic_providers_without_modifying_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    fs::write(
        &path,
        json!({
            "default_provider": "kimi",
            "providers": [
                {"id":"openai","kind":"openai-compatible",
                 "base_url":"https://api.openai.com/v1","model":"gpt-4.1-mini"},
                {"id":"glm","kind":"openai-compatible",
                 "base_url":"https://gateway.example/v1","model":"glm-custom"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let original = fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .arg("models")
        .env("ATLAS_CONFIG", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("kimi (default): openai-compatible / kimi-k2.6"),
        "{stdout}"
    );
    assert!(
        stdout.contains("glm: openai-compatible / glm-custom @ https://gateway.example/v1"),
        "{stdout}"
    );
    for id in ["deepseek", "minimax"] {
        assert!(
            stdout.contains(&format!("{id}: openai-compatible")),
            "{stdout}"
        );
    }
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn four_domestic_provider_ids_use_existing_authenticated_chat_pipeline() {
    for (id, model) in [
        ("deepseek", "deepseek-flash"),
        ("kimi", "kimi-k2.6"),
        ("glm", "glm-4.7"),
        ("minimax", "MiniMax-M2.7"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            let (mut client, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "model API was not called");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept model API call: {error}"),
                }
            };
            let (headers, body) = read_request(&mut client);
            assert!(
                headers.starts_with("POST /v1/chat/completions HTTP/1.1"),
                "{headers}"
            );
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-key-123")
            );
            assert_eq!(body["model"], model);
            assert_eq!(body["messages"][0]["role"], "system");
            assert_eq!(body["messages"][1]["role"], "user");
            assert!(
                body["tools"].is_array(),
                "agent tools must reach provider API"
            );
            assert_ne!(
                body["stream"], true,
                "bundled profiles default to nonstream"
            );
            let reply = json!({
                "choices":[{
                    "message":{"role":"assistant","content":"provider-ok"},
                    "finish_reason":"stop"
                }],
                "usage":{"prompt_tokens":13,"completion_tokens":2,"total_tokens":15}
            })
            .to_string();
            write!(client,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(), reply
            ).unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        fs::write(
            &config,
            json!({
                "default_provider":id,
                "providers":[{
                    "id":id, "kind":"openai-compatible",
                    "base_url":format!("http://{address}/v1"),
                    "model":model, "api_key_env":"TEST_DOMESTIC_API_KEY"
                }]
            })
            .to_string(),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
            .args([
                "--workspace", dir.path().to_str().unwrap(),
                "--provider", id, "--approval", "read-only",
                "run", "Reply simply",
            ])
            .env("ATLAS_CONFIG", &config)
            .env("TEST_DOMESTIC_API_KEY", "test-key-123")
            .output().unwrap();
        server.join().unwrap();
        assert!(
            output.status.success(),
            "{id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("provider-ok"),
            "{id} output: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}


#[test]
fn deepseek_reasoning_is_replayed_with_tool_result_in_next_api_turn() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "reasoning-roundtrip-file\n").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        for turn in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(20);
            let (mut socket, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "missing model API turn {turn}");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("API accept: {error}"),
                }
            };
            let (_headers, request) = read_request(&mut socket);
            let response = if turn == 0 {
                json!({"choices":[{"message":{
                    "role":"assistant",
                    "content":null,
                    "reasoning_content":"I will inspect the file before answering",
                    "tool_calls":[{
                        "id":"deepseek_call_1","type":"function",
                        "function":{"name":"read_file","arguments":"{\\"path\\":\\"sample.txt\\"}"}
                    }]
                },"finish_reason":"tool_calls"}]})
            } else {
                let messages = request["messages"].as_array().unwrap();
                let assistant = messages.iter().find(|m| {
                    m["role"] == "assistant" && m["tool_calls"].is_array()
                }).expect("missing prior assistant tool message");
                assert_eq!(assistant["reasoning_content"], "I will inspect the file before answering");
                assert_eq!(assistant["tool_calls"][0]["id"], "deepseek_call_1");
                let tool_result = messages.iter().find(|m| m["role"] == "tool")
                    .expect("missing tool result");
                assert_eq!(tool_result["tool_call_id"], "deepseek_call_1");
                assert!(tool_result["content"].as_str().unwrap().contains("reasoning-roundtrip-file"));
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"reasoning roundtrip completed"},"finish_reason":"stop"}]})
            };
            let payload = response.to_string();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            )
            .unwrap();
        }
    });
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"deepseek","providers":[{
            "id":"deepseek","kind":"openai-compatible",
            "base_url":format!("http://{address}/v1"),
            "model":"deepseek-flash","api_key_env":"TEST_DOMESTIC_API_KEY"
        }]})
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "--workspace", dir.path().to_str().unwrap(),
            "--approval", "read-only", "run", "Inspect sample.txt",
        ])
        .env("ATLAS_CONFIG", &config)
        .env("TEST_DOMESTIC_API_KEY", "test-key-123")
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("reasoning roundtrip completed"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let session_dir = dir.path().join(".atlas/sessions");
    let session_file = fs::read_dir(session_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = fs::read_to_string(session_file).unwrap();
    assert!(saved.contains("I will inspect the file before answering"));
}
