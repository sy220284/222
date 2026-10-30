//! Offline transport checks for bundled API model IDs and legacy settings.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::Duration,
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
    let length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().unwrap())
    }).expect("Content-Length");
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
    fs::write(&path, json!({
        "default_provider": "kimi",
        "providers": [
            {"id":"openai","kind":"openai-compatible",
             "base_url":"https://api.openai.com/v1","model":"gpt-4.1-mini"},
            {"id":"glm","kind":"openai-compatible",
             "base_url":"https://gateway.example/v1","model":"glm-custom"}
        ]
    }).to_string()).unwrap();
    let original = fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .arg("models").env("ATLAS_CONFIG", &path).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("kimi (default): openai-compatible / kimi-k2.6"), "{stdout}");
    assert!(stdout.contains("glm: openai-compatible / glm-custom @ https://gateway.example/v1"), "{stdout}");
    for id in ["deepseek", "minimax"] {
        assert!(stdout.contains(&format!("{id}: openai-compatible")), "{stdout}");
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
        let server = thread::spawn(move || {
            let (mut client, _) = listener.accept().expect("accept model API call");
            let (headers, body) = read_request(&mut client);
            assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1"), "{headers}");
            assert!(headers.to_ascii_lowercase().contains("authorization: bearer test-key-123"));
            assert_eq!(body["model"], model);
            assert_eq!(body["messages"][0]["role"], "system");
            assert_eq!(body["messages"][1]["role"], "user");
            assert!(body["tools"].is_array(), "agent tools must reach provider API");
            assert_ne!(body["stream"], true, "bundled profiles default to nonstream");
            let reply = json!({
                "choices":[{
                    "message":{"role":"assistant","content":"provider-ok"},
                    "finish_reason":"stop"
                }],
                "usage":{"prompt_tokens":13,"completion_tokens":2,"total_tokens":15}
            }).to_string();
            write!(client,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(), reply
            ).unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        fs::write(&config, json!({
            "default_provider":id,
            "providers":[{
                "id":id, "kind":"openai-compatible",
                "base_url":format!("http://{address}/v1"),
                "model":model, "api_key_env":"TEST_DOMESTIC_API_KEY"
            }]
        }).to_string()).unwrap();
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
        assert!(output.status.success(), "{id}: {}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("provider-ok"),
            "{id} output: {}", String::from_utf8_lossy(&output.stdout));
    }
}
