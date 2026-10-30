use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn hook(event: &str, response: Value) -> Value {
    let output = response.to_string().replace('\'', "'\"'\"'");
    #[cfg(unix)]
    let (shell, command) = ("bash", format!("printf '%s' '{output}'"));
    #[cfg(windows)]
    let (shell, command) = ("powershell", format!("Write-Output '{output}'"));
    json!({"event":event,"matcher":"^write_file$", "shell":shell,
        "command":command,"timeout_seconds":5})
}

fn run_tool_turn(
    root: &std::path::Path,
    streaming: bool,
    hooks: Vec<Value>,
    arguments: Value,
) -> Vec<Value> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for turn in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut conn = loop {
                match listener.accept() {
                    Ok((conn, _)) => break conn,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "missing model turn {turn}");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept model request: {error}"),
                }
            };
            conn.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut data = Vec::new();
            let (body_start, length) = loop {
                let mut bytes = [0; 4096];
                let n = conn.read(&mut bytes).unwrap();
                assert_ne!(n, 0);
                data.extend_from_slice(&bytes[..n]);
                if let Some(index) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&data[..index]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break (index + 4, length);
                }
            };
            while data.len() < body_start + length {
                let mut bytes = [0; 4096];
                let n = conn.read(&mut bytes).unwrap();
                assert_ne!(n, 0);
                data.extend_from_slice(&bytes[..n]);
            }
            requests.push(
                serde_json::from_slice::<Value>(&data[body_start..body_start + length]).unwrap(),
            );
            let delta = if turn == 0 {
                json!({"role":"assistant","tool_calls":[{"index":0,"id":"write-once",
                    "type":"function","function":{"name":"write_file", "arguments":
                        arguments.to_string()}}]})
            } else {
                json!({"role":"assistant","content":"Done"})
            };
            let body = format!(
                "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                json!({"choices":[{"index":0,"delta":delta,"finish_reason":null}]}),
                json!({"choices":[{"index":0,"delta":{},"finish_reason":if turn==0 {"tool_calls"} else {"stop"}}]})
            );
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        requests
    });
    let config = root.join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"mock","stream":true,
        "base_url":format!("http://{address}/v1")}],"hooks":hooks})
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args([
            "--workspace",
            root.to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Write a file",
        ])
        .env("ATLAS_CONFIG", &config)
        .env(
            "ATLAS_STREAMING_TOOL_EXECUTION",
            if streaming { "1" } else { "0" },
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Done"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let requests = server.join().unwrap();
    requests
}

#[test]
fn forced_hook_approval_blocks_real_streamed_write_even_with_full_approval() {
    for streaming in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let requests = run_tool_turn(
            root.path(),
            streaming,
            vec![hook(
                "PreToolUse",
                json!({
            "hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask"}}),
            )],
            json!({"path":"original.txt","content":"original"}),
        );
        assert!(
            !root.path().join("original.txt").exists(),
            "streaming={streaming} bypassed hook approval"
        );
        let result = requests[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["tool_call_id"] == "write-once")
            .unwrap();
        assert!(
            result["content"]
                .as_str()
                .unwrap()
                .contains("PreToolUse hook requires interactive approval"),
            "{result}"
        );
    }
}

#[test]
fn streamed_rewritten_input_and_hook_context_reach_disk_and_next_model_turn() {
    for streaming in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let requests = run_tool_turn(
            root.path(),
            streaming,
            vec![
                hook(
                    "PreToolUse",
                    json!({"hookSpecificOutput":{"hookEventName":"PreToolUse",
                "updatedInput":{"path":"rewritten.txt","content":"effective"},
                "additionalContext":"PRE_CONTEXT"}}),
                ),
                hook(
                    "PostToolUse",
                    json!({"hookSpecificOutput":{"hookEventName":"PostToolUse",
                "additionalContext":"POST_CONTEXT"}}),
                ),
            ],
            json!({"path":"original.txt","content":"original"}),
        );
        assert!(!root.path().join("original.txt").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("rewritten.txt")).unwrap(),
            "effective"
        );
        let result = requests[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["tool_call_id"] == "write-once")
            .unwrap();
        let content = result["content"].as_str().unwrap();
        assert!(
            content.contains("[PreToolUse hook context] PRE_CONTEXT"),
            "streaming={streaming}: {content}"
        );
        assert!(
            content.contains("[Local hook context] POST_CONTEXT"),
            "streaming={streaming}: {content}"
        );
    }
}

#[test]
fn invalid_streamed_input_never_runs_hooks_or_side_effects() {
    for streaming in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let requests = run_tool_turn(
            root.path(),
            streaming,
            vec![
                hook(
                    "PreToolUse",
                    json!({"hookSpecificOutput":{
                "hookEventName":"PreToolUse", "additionalContext":"UNEXPECTED_PRE_HOOK"}}),
                ),
                hook(
                    "PostToolUseFailure",
                    json!({"hookSpecificOutput":{
                "hookEventName":"PostToolUseFailure", "additionalContext":"UNEXPECTED_POST_HOOK"}}),
                ),
            ],
            json!([]),
        );
        let result = requests[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["tool_call_id"] == "write-once")
            .unwrap();
        let content = result["content"].as_str().unwrap();
        assert!(
            content.contains("Tool arguments must be a JSON object"),
            "{content}"
        );
        assert!(
            !content.contains("UNEXPECTED_"),
            "streaming={streaming}: {content}"
        );
        assert!(!root.path().join("original.txt").exists());
    }
}
