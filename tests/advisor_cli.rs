//! Integration test: the configured advisor must use its own provider and
//! feed the result back into the main model's actual next request.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::Duration,
};

fn receive(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut data = Vec::new();
    let mut buf = [0_u8; 8192];
    let end = loop {
        let n = stream.read(&mut buf).unwrap();
        assert!(n != 0, "HTTP request ended before its headers");
        data.extend_from_slice(&buf[..n]);
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let headers = String::from_utf8_lossy(&data[..end]);
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    while data.len() - end < length {
        let n = stream.read(&mut buf).unwrap();
        assert!(n != 0, "HTTP body ended early");
        data.extend_from_slice(&buf[..n]);
    }
    serde_json::from_slice(&data[end..end + length]).unwrap()
}

fn reply(stream: &mut TcpStream, content: Value) {
    let text = content.to_string();
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).unwrap();
    stream.flush().unwrap();
}

#[test]
fn advisor_calls_secondary_provider_with_history_and_returns_its_result() {
    let primary = TcpListener::bind("127.0.0.1:0").unwrap();
    let advisor = TcpListener::bind("127.0.0.1:0").unwrap();
    let primary_addr = primary.local_addr().unwrap();
    let advisor_addr = advisor.local_addr().unwrap();
    let primary_thread = thread::spawn(move || {
        let (mut conn, _) = primary.accept().unwrap();
        let first = receive(&mut conn);
        assert_eq!(first["model"], "main-model");
        assert!(first["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| { tool["function"]["name"] == "advisor" }));
        reply(
            &mut conn,
            json!({"choices":[{"message":{"role":"assistant", "content":null,
            "tool_calls":[{"id":"advisor-1","type":"function","function":{
                "name":"advisor", "arguments":"{}"}}]},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":11,"completion_tokens":5}}),
        );
        let (mut conn, _) = primary.accept().unwrap();
        let second = receive(&mut conn);
        assert!(second["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| {
                message["role"] == "tool"
                    && message["content"]
                        .as_str()
                        .is_some_and(|text| text.contains("Important issue from advisor"))
            }));
        reply(
            &mut conn,
            json!({"choices":[{"message":{
            "role":"assistant","content":"Main answer improved"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":20,"completion_tokens":5}}),
        );
    });
    let advisor_thread = thread::spawn(move || {
        let (mut conn, _) = advisor.accept().unwrap();
        let request = receive(&mut conn);
        assert_eq!(request["model"], "review-model");
        assert!(request.get("tools").is_none());
        assert!(request["messages"].as_array().unwrap().iter().any(|msg| {
            msg["role"] == "user"
                && msg["content"]
                    .as_str()
                    .is_some_and(|text| text.contains("Find issues"))
        }));
        reply(
            &mut conn,
            json!({"choices":[{"message":{"role":"assistant",
            "content":"Important issue from advisor"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":9,"completion_tokens":4}}),
        );
    });
    let root = tempfile::tempdir().unwrap();
    let settings = root.path().join("settings.json");
    fs::write(
        &settings,
        json!({"default_provider":"main","advisorModel":"review",
        "providers":[
            {"id":"main","kind":"openai-compatible","model":"main-model",
             "base_url":format!("http://{primary_addr}/v1"),"stream":false},
            {"id":"review","kind":"openai-compatible","model":"review-model",
             "base_url":format!("http://{advisor_addr}/v1"),"stream":false}
        ]})
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .env("ATLAS_CONFIG", &settings)
        .arg("--workspace")
        .arg(root.path())
        .args(["run", "Find issues"])
        .output()
        .unwrap();
    primary_thread.join().unwrap();
    advisor_thread.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Main answer improved"));
    let session_file = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved: Value = serde_json::from_slice(&fs::read(session_file).unwrap()).unwrap();
    assert!(saved["usage"]["total_tokens"].as_u64().unwrap_or(0) >= 54);
}
