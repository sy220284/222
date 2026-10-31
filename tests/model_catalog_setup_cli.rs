//! CLI integration of the complete brand, credential, live catalog, probe and save workflow.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn receive(stream: &mut std::net::TcpStream) -> (String, Value) {
    // A nonblocking listener can yield nonblocking accepted streams on
    // Windows. Request parsing needs a blocking stream with read timeout.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let end = loop {
        let n = stream.read(&mut buffer).expect("Read model request");
        assert!(n > 0);
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(i) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while bytes.len() < end + length {
        let n = stream.read(&mut buffer).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&buffer[..n]);
    }
    let data = if length == 0 {
        json!({})
    } else {
        serde_json::from_slice(&bytes[end..end + length]).unwrap()
    };
    (headers, data)
}

#[test]
fn interactive_setup_discovers_and_probes_then_persists_model_and_context() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        for round in 0..2 {
            let start = std::time::Instant::now();
            let (mut socket, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(25),
                            "model API not called"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("model API accept failed: {error}"),
                }
            };
            let (headers, data) = receive(&mut socket);
            assert!(headers
                .to_ascii_lowercase()
                .contains("authorization: bearer local-key-test"));
            let response = if round == 0 {
                assert!(headers.starts_with("GET /v1/models HTTP/1.1"));
                json!({"data":[
                    {"id":"model-new","name":"Latest API model",
                     "context_window":200000,"max_output_tokens":8192,
                     "input_modalities":["text","image"],
                     "effort":{"supported_levels":["low","high"]}}
                ]})
            } else {
                assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1"));
                assert_eq!(data["model"], "model-new");
                assert_eq!(data["max_tokens"], 32);
                json!({"choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}]})
            };
            let body = response.to_string();
            write!(socket,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body).unwrap();
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"custom",
            "providers":[{
                "id":"custom","kind":"openai-compatible",
                "base_url":format!("http://127.0.0.1:{port}/v1"),
                "model":"older-model","api_key_env":"CUSTOM_MODEL_TEST_KEY",
            }]
        })
        .to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["models", "setup", "custom"])
        .env("ATLAS_CONFIG", &config)
        .env("CUSTOM_MODEL_TEST_KEY", "local-key-test")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"1\n").unwrap();
    let output = child.wait_with_output().unwrap();
    worker.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("model-new"));
    let persisted: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    let selected = persisted["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "custom")
        .unwrap();
    assert_eq!(persisted["default_provider"], "custom");
    assert_eq!(selected["model"], "model-new");
    assert_eq!(selected["tuning"]["context_window"], 200000);
    assert_eq!(selected["tuning"]["max_output_tokens"], Value::Null);
    assert!(!fs::read_to_string(&config)
        .unwrap()
        .contains("local-key-test"));

    let tune = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "models",
            "tune",
            "custom",
            "--temperature",
            "0.35",
            "--top-p",
            "0.95",
            "--max-output-tokens",
            "8192",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        tune.status.success(),
        "{}",
        String::from_utf8_lossy(&tune.stderr)
    );
    let updated: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    let value = updated["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "custom")
        .unwrap();
    assert_eq!(value["tuning"]["temperature"], 0.35);
    assert_eq!(value["tuning"]["top_p"], 0.95);
    assert_eq!(value["tuning"]["max_output_tokens"], 8192);
}
