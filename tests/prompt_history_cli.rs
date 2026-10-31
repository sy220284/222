use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
#[test]
fn chat_history_persists_across_real_cli_processes_and_model_http() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("project");
    fs::create_dir_all(&workspace).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let config = root.path().join("config").join("config.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        serde_json::json!({
            "default_provider":"test",
            "providers":[{"id":"test","kind":"openai-compatible",
                "base_url":format!("http://{addr}/v1"),"model":"test-model"}]
        })
        .to_string(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut received = Vec::new();
        while received.len() < 2 && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    // Windows accepts a socket in the listener's nonblocking
                    // mode. Switch the accepted connection to blocking reads
                    // before applying a timeout; otherwise read() can fail
                    // immediately with WSAEWOULDBLOCK despite incoming data.
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_secs(4)))
                        .unwrap();
                    let mut input = Vec::new();
                    loop {
                        let mut buffer = [0u8; 8192];
                        let n = socket.read(&mut buffer).unwrap();
                        assert!(n > 0, "request closed early");
                        input.extend_from_slice(&buffer[..n]);
                        let Some(end) = input.windows(4).position(|w| w == b"\r\n\r\n") else {
                            continue;
                        };
                        let headers = String::from_utf8_lossy(&input[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                if key.eq_ignore_ascii_case("content-length") {
                                    value.trim().parse::<usize>().ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        if input.len() < end + 4 + length {
                            continue;
                        }
                        let body: serde_json::Value =
                            serde_json::from_slice(&input[end + 4..end + 4 + length]).unwrap();
                        received.push(body);
                        let payload = serde_json::json!({"choices":[{
                            "message":{"role":"assistant","content":"ack"},
                            "finish_reason":"stop"}],"usage":{
                            "prompt_tokens":4,"completion_tokens":1,"total_tokens":5
                        }})
                        .to_string();
                        let reply=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",payload.len());
                        socket.write_all(reply.as_bytes()).unwrap();
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("HTTP accept: {e}"),
            }
        }
        received
    });
    let run = |input: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
            .args(["--workspace", workspace.to_str().unwrap(), "chat"])
            .env("ATLAS_CONFIG", &config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let first = run("first prompt\n/history first\n/exit\n");
    assert!(first.contains("ack"));
    assert!(first.contains("first prompt"));
    let second = run("second prompt\n/history first\n/exit\n");
    assert!(second.contains("ack"));
    assert!(
        second.contains("first prompt"),
        "history not shared across processes: {second}"
    );
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].to_string().contains("first prompt"));
    assert!(requests[1].to_string().contains("second prompt"));
    let history = fs::read_to_string(config.parent().unwrap().join("history.jsonl")).unwrap();
    assert!(history.contains("first prompt") && history.contains("second prompt"));
    assert!(!history.contains("ATLAS_CONFIG"));
}
