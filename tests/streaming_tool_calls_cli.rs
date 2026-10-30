//! Real SDK subprocess plus local OpenAI-compatible SSE endpoint.
//! Source parity: early tool execution must resume the model, preserve tool
//! blocks in the transcript, and return each tool result exactly once.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn read_request(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let end = loop {
        let count = stream.read(&mut buf).expect("read request headers");
        assert!(count > 0, "closed before request headers");
        data.extend_from_slice(&buf[..count]);
        if let Some(i) = data.windows(4).position(|part| part == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let header = String::from_utf8_lossy(&data[..end]);
    let length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .expect("Content-Length missing");
    while data.len() < end + length {
        let count = stream.read(&mut buf).expect("read request body");
        assert!(count > 0, "closed before request body");
        data.extend_from_slice(&buf[..count]);
    }
    serde_json::from_slice(&data[end..end + length]).unwrap()
}

fn respond_sse(stream: &mut TcpStream, events: &[Value]) {
    let body = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect::<String>()
        + "data: [DONE]\n\n";
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body,
    )
    .unwrap();
    stream.flush().unwrap();
}

fn accept_before(listener: &TcpListener, deadline: Instant) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "model failed to resume after tool results");
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("accept mock request: {e}"),
        }
    }
}

#[test]
fn streamed_tool_calls_execute_once_and_resume_sdk_turn() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut first = accept_before(&listener, Instant::now() + Duration::from_secs(15));
        let request = read_request(&mut first);
        assert_eq!(request["stream"], true);
        respond_sse(
            &mut first,
            &[
                json!({"choices":[{"delta":{"role":"assistant","tool_calls":[{
                    "index":0,"id":"call-read-a","type":"function",
                    "function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}
                }]}}]}),
                json!({"choices":[{"delta":{"tool_calls":[{
                    "index":1,"id":"call-read-b","type":"function",
                    "function":{"name":"read_file","arguments":"{\"path\":\"b.txt\"}"}
                }]}}]}),
                json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
            ],
        );
        let mut second = accept_before(&listener, Instant::now() + Duration::from_secs(15));
        let next = read_request(&mut second);
        respond_sse(
            &mut second,
            &[json!({"choices":[{"delta":{"role":"assistant","content":"Done"},"finish_reason":"stop"}]})],
        );
        next
    });
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "contents from A").unwrap();
    fs::write(dir.path().join("b.txt"), "contents from B").unwrap();
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test-model",
            "base_url":format!("http://{addr}/v1"),"stream":true
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .env("ATLAS_STREAMING_TOOL_EXECUTION", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"type":"user","content":"Read both local files"})
        )
        .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("SDK subprocess did not finish after tool results");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    let second = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = second["messages"].as_array().unwrap();
    let assistant = messages
        .iter()
        .find(|message| message["role"] == "assistant" && message["tool_calls"].is_array())
        .expect("first assistant turn must retain tool blocks");
    let blocks = assistant["tool_calls"].as_array().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0]["id"], "call-read-a");
    assert_eq!(blocks[1]["id"], "call-read-b");
    let results: Vec<&Value> = messages
        .iter()
        .filter(|message| message["role"] == "tool")
        .collect();
    assert_eq!(results.len(), 2, "no duplicate or missing tool results");
    assert_eq!(results[0]["tool_call_id"], "call-read-a");
    assert_eq!(results[1]["tool_call_id"], "call-read-b");
    assert!(results[0]["content"].as_str().unwrap().contains("contents from A"));
    assert!(results[1]["content"].as_str().unwrap().contains("contents from B"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Done"));
}
