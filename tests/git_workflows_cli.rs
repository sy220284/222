use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn read_http(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut data = Vec::new();
    loop {
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "HTTP request ended before its body");
        data.extend_from_slice(&buf[..n]);
        if let Some(i) = data.windows(4).position(|x| x == b"\r\n\r\n") {
            let h = String::from_utf8_lossy(&data[..i]);
            let length = h
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if data.len() >= i + 4 + length {
                return serde_json::from_slice(&data[i + 4..i + 4 + length]).unwrap();
            }
        }
    }
}

fn mock_server() -> (String, thread::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_http(&mut stream);
        let body=json!({"choices":[{"message":{"role":"assistant","content":"Workflow acknowledged"},"finish_reason":"stop"}]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        stream.flush().unwrap();
        request
    });
    (format!("http://{address}/v1"), handle)
}

fn settings(root: &std::path::Path, url: &str) -> std::path::PathBuf {
    let p = root.join("model.json");
    fs::write(&p,json!({"default_provider":"mock","providers":[{"id":"mock","kind":"openai-compatible","model":"test","base_url":url,"stream":false}]}).to_string()).unwrap();
    p
}

#[test]
fn review_cli_sends_source_review_prompt_through_actual_agent_and_provider() {
    let root = tempfile::tempdir().unwrap();
    let (url, thread) = mock_server();
    let config = settings(root.path(), &url);
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "review",
            "42",
        ])
        .env("ATLAS_CONFIG", config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Workflow acknowledged"));
    let request = thread.join().unwrap();
    let prompt = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(prompt.contains("gh pr diff"), "{prompt}");
    assert!(prompt.contains("PR number: 42"), "{prompt}");
}

#[test]
fn slash_review_is_handled_as_command_not_literal_chat_prompt() {
    let root = tempfile::tempdir().unwrap();
    let (url, thread) = mock_server();
    let config = settings(root.path(), &url);
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "chat",
        ])
        .env("ATLAS_CONFIG", config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/review 12\n/exit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Workflow acknowledged"));
    let request = thread.join().unwrap();
    let input = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(input.contains("PR number: 12"));
    assert!(!input.contains("\"/review 12\""));
}
