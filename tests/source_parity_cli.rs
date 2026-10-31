//! Source-parity end-to-end smoke cases: real CLI and mock provider request.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn executable() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

fn config(root: &std::path::Path, address: impl std::fmt::Display) -> std::path::PathBuf {
    let file = root.join("model.json");
    fs::write(
        &file,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{address}/v1"),"stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    file
}

#[test]
fn original_output_style_notice_has_no_model_or_transcript_side_effect() {
    let root = tempfile::tempdir().unwrap();
    let configuration = config(root.path(), "127.0.0.1:1");
    let mut process = executable()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &configuration)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(b"/output-style\n/session\n/exit\n")
        .unwrap();
    let result = process.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    const EXPECTED:&str="/output-style has been deprecated. Use /config to change your output style, or set it in your settings file. Changes take effect on the next session.";
    let output = String::from_utf8(result.stdout).unwrap();
    assert_eq!(output.matches(EXPECTED).count(), 1, "{output}");
    // The original callback uses display=system; the notice is not a user message.
    let sessions = root.path().join(".atlas/sessions");
    if sessions.is_dir() {
        for record in fs::read_dir(&sessions).unwrap() {
            let item: Value =
                serde_json::from_slice(&fs::read(record.unwrap().path()).unwrap()).unwrap();
            assert!(item["messages"].as_array().unwrap().is_empty(), "{item}");
        }
    }
}

#[test]
fn original_glob_prompt_is_sent_verbatim_in_real_model_request() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let configuration = config(root.path(), listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut socket = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "Mock model was never called");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("Mock server error: {error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buf = [0u8; 4096];
            let count = socket.read(&mut buf).unwrap();
            assert!(count > 0, "Incomplete mock model request");
            request.extend_from_slice(&buf[..count]);
            if let Some(index) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..index]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= index + 4 + length {
                    break;
                }
            }
        }
        let sep = request.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
        let input: Value = serde_json::from_slice(&request[sep + 4..]).unwrap();
        let body = json!({"choices":[{"message":{"role":"assistant","content":"ready"},
            "finish_reason":"stop"}]})
        .to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        socket.flush().unwrap();
        input
    });
    let output = executable()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "run",
            "Inspect files",
        ])
        .env("ATLAS_CONFIG", &configuration)
        .output()
        .unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let glob = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == "glob_files")
        .expect("missing live glob tool");
    const SOURCE:&str="- Fast file pattern matching tool that works with any codebase size\n- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"\n- Returns matching file paths sorted by modification time\n- Use this tool when you need to find files by name patterns\n- When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead";
    assert_eq!(glob["function"]["description"], SOURCE);
    assert_eq!(
        glob["function"]["parameters"]["required"],
        json!(["pattern"])
    );
}
