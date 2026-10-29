use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::Duration,
};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

#[test]
fn cli_initialization_and_model_listing_work_offline() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    let init = binary()
        .args(["init"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(init.status.success(), "{:?}", init);
    assert!(config.is_file());
    let list = binary()
        .args(["models"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8(list.stdout).unwrap();
    assert!(stdout.contains("local (default)"));
    assert!(stdout.contains("gemini"));
}

#[test]
fn deep_link_cli_preserves_empty_params_and_validates_unicode() {
    let parsed = binary()
        .args(["parse-deep-link", "atlas-cli://open?cwd=&repo=&q="])
        .output()
        .unwrap();
    assert!(parsed.status.success(), "{:?}", parsed);
    let action: Value = serde_json::from_slice(&parsed.stdout).unwrap();
    assert_eq!(action["cwd"], "");
    assert_eq!(action["repo"], "");
    assert!(action["query"].is_null());

    let built = binary().args(["build-deep-link"]).output().unwrap();
    assert!(built.status.success(), "{:?}", built);
    assert_eq!(
        String::from_utf8(built.stdout).unwrap().trim(),
        "atlas-cli://open"
    );

    let sanitized = binary()
        .args(["parse-deep-link", "atlas-cli://open?q=Ｈi%E2%80%8B"])
        .output()
        .unwrap();
    assert!(sanitized.status.success(), "{:?}", sanitized);
    let result: Value = serde_json::from_slice(&sanitized.stdout).unwrap();
    assert_eq!(result["query"], "Hi");
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    let mut data = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..count]);
        if let Some(index) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&data[..index]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if data.len() >= index + 4 + length {
                break;
            }
        }
    }
    String::from_utf8(data).unwrap()
}

#[test]
fn model_capacity_retry_keeps_one_logical_tool_turn_and_session() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sample.txt"), "SECRET_TOKEN_READ_ONCE\n").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let (status, payload) = match index {
                0 => (
                    "503 Service Unavailable",
                    json!({"error":{"message":"capacity"}}),
                ),
                1 => (
                    "429 Too Many Requests",
                    json!({"error":{"message":"rate limit"}}),
                ),
                2 => (
                    "200 OK",
                    json!({"choices":[{"message":{
                    "role":"assistant","content":null,"tool_calls":[{
                        "id":"one_tool_call","type":"function","function":{
                            "name":"read_file","arguments":"{\"path\":\"sample.txt\"}"
                        }
                    }]
                },"finish_reason":"tool_calls"}]}),
                ),
                _ => (
                    "200 OK",
                    json!({"choices":[{"message":{
                    "role":"assistant","content":"Retried request completed once."
                },"finish_reason":"stop"}]}),
                ),
            };
            let body = payload.to_string();
            write!(stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nRetry-After: 0\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("model.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{addr}/v1"),"stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Inspect sample.txt",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Retried request completed once."));
    let bodies = requests
        .iter()
        .map(|raw| raw.split_once("\r\n\r\n").unwrap().1)
        .collect::<Vec<_>>();
    assert_eq!(bodies[0], bodies[1], "503 retry changed the request body");
    assert_eq!(bodies[1], bodies[2], "429 retry changed the request body");
    assert_eq!(
        bodies[3].matches("SECRET_TOKEN_READ_ONCE").count(),
        1,
        "tool output must be inserted exactly once after retries"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("sample.txt")).unwrap(),
        "SECRET_TOKEN_READ_ONCE\n"
    );
    let session_dir = root.path().join(".atlas/sessions");
    let session_file = fs::read_dir(session_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let session = fs::read_to_string(session_file).unwrap();
    assert_eq!(
        session.matches("one_tool_call").count(),
        2,
        "assistant tool call and its result must be saved once each"
    );
}

#[test]
fn provider_explicit_no_retry_and_invalid_request_do_not_replay() {
    for (status, extra) in [
        ("503 Service Unavailable", "x-should-retry: false\r\n"),
        ("400 Bad Request", ""),
    ] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            thread::sleep(Duration::from_millis(35));
            let body = if status.starts_with("400") {
                "invalid model parameters".to_string()
            } else {
                json!({"error":{"message":"not retryable"}}).to_string()
            };
            write!(stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()).unwrap();
            stream.flush().unwrap();
            request
        });
        let config = root.path().join("model.json");
        fs::write(
            &config,
            json!({"default_provider":"mock","providers":[{
                "id":"mock","kind":"openai-compatible","model":"test",
                "base_url":format!("http://{addr}/v1"),"stream":false
            }]})
            .to_string(),
        )
        .unwrap();
        let output = binary()
            .args([
                "--workspace",
                root.path().to_str().unwrap(),
                "run",
                "Try model",
            ])
            .env("ATLAS_CONFIG", &config)
            .output()
            .unwrap();
        let request = server.join().unwrap();
        assert!(request.contains("Try model"));
        assert!(
            !output.status.success(),
            "non-retryable model error unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&status[..3]),
            "expected original HTTP error, got {stderr}"
        );
        let session_file = fs::read_dir(root.path().join(".atlas/sessions"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let saved: Value = serde_json::from_slice(&fs::read(session_file).unwrap()).unwrap();
        assert!(
            saved["cost"]["total_api_duration_ms"].as_u64().unwrap_or(0) >= 20,
            "failed API time must survive session persistence: {saved}"
        );
        assert!(
            saved["cost"]["total_wall_duration_ms"]
                .as_u64()
                .unwrap_or(0)
                >= 20,
            "failed turn wall time must survive session persistence: {saved}"
        );
    }
}

#[test]
fn truncated_stream_cannot_execute_a_complete_but_unconfirmed_tool_call() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        let event = json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"id":"unconfirmed_write","type":"function",
            "function":{"name":"write_file","arguments":
                "{\"path\":\"must-not-exist.txt\",\"content\":\"UNCONFIRMED\"}"}
        }]}}]});
        let body = format!("data: {}\n\n", event);
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()).unwrap();
        stream.flush().unwrap();
        request
    });
    let config = root.path().join("model.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{addr}/v1"),"stream":true
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Create a file only when complete",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let request = server.join().unwrap();
    assert!(request.contains("Create a file only when complete"));
    assert!(
        !output.status.success(),
        "truncated stream was treated as success"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("completion marker"));
    assert!(
        !root.path().join("must-not-exist.txt").exists(),
        "an unconfirmed streamed tool call must never touch the filesystem"
    );
    let session_file = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = fs::read_to_string(session_file).unwrap();
    assert!(
        !saved.contains("unconfirmed_write"),
        "partial assistant tool call must not be committed to session history"
    );
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
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "base_url":format!("http://{addr}/v1"),"model":"test-model"}]
        })
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            dir.path().to_str().unwrap(),
            "run",
            "Inspect example.txt",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    fs::write(
        &config,
        json!({
            "default_provider":"gm",
            "providers":[{"id":"gm","kind":"gemini",
                "base_url":format!("http://{address}/v1beta"),
                "model":"example","api_key_env":"ATLAS_TEST_GEMINI_KEY"}]
        })
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            dir.path().to_str().unwrap(),
            "run",
            "Read example.txt",
        ])
        .env("ATLAS_CONFIG", &config)
        .env("ATLAS_TEST_GEMINI_KEY", "fake-local-key")
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Gemini read verified."));
    assert!(requests[0].contains("functionDeclarations"));
    assert!(requests[1].contains("functionResponse"));
    assert!(requests[1].contains("gemini test file"));
}

#[test]
fn subagent_runs_a_separate_model_conversation_and_returns_to_parent() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let data = read_request(&mut stream);
            requests.push(data);
            let reply = match step {
                0 => json!({"choices":[{"message":{
                    "role":"assistant","content":null,
                    "tool_calls":[{"id":"parent_spawn","type":"function",
                        "function":{"name":"spawn_agent",
                            "arguments":json!({"prompt":"Analyze workspace","isolation":"none"}).to_string()
                        }}]
                },"finish_reason":"tool_calls"}]}),
                1 => json!({"choices":[{"message":{
                    "role":"assistant","content":"Subagent completed analysis."
                },"finish_reason":"stop"}]}),
                _ => json!({"choices":[{"message":{
                    "role":"assistant","content":"Delegation verified."
                },"finish_reason":"stop"}]}),
            };
            let mut reply = reply;
            reply["usage"] = json!({"prompt_tokens":20,"completion_tokens":10,
                "total_tokens":30});
            let body = reply.to_string();
            write!(stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("provider.json");
    fs::write(
        &config,
        json!({"default_provider":"test","providers":[{
            "id":"test","kind":"openai-compatible","model":"mock",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Delegate analysis",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Delegation verified."));
    assert!(requests[1].contains("Analyze workspace"));
    assert!(!requests[1].contains("spawn_agent"));
    assert!(requests[2].contains("Subagent completed analysis."));
    assert_eq!(
        fs::read_dir(root.path().join(".atlas/sessions"))
            .unwrap()
            .count(),
        2
    );
    let files = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .map(|v| v.unwrap().path())
        .collect::<Vec<_>>();
    let sessions = files
        .iter()
        .map(|file| serde_json::from_slice::<Value>(&fs::read(file).unwrap()).unwrap())
        .collect::<Vec<_>>();
    let parent = sessions
        .iter()
        .find(|session| {
            session["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .contains("Delegate analysis")
        })
        .expect("parent session");
    let child = sessions
        .iter()
        .find(|session| {
            session["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .contains("Analyze workspace")
        })
        .expect("child session");
    assert_eq!(child["usage"]["input_tokens"], 20);
    assert_eq!(parent["usage"]["input_tokens"], 60);
    assert_eq!(parent["usage"]["output_tokens"], 30);
    assert_eq!(parent["cost"]["models"][0]["usage"]["input_tokens"], 60);
    assert_eq!(parent["cost"]["models"][0]["has_unpriced_usage"], true);
}

#[test]
fn stop_hook_block_causes_one_additional_model_round() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut messages = Vec::new();
        for i in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            messages.push(read_request(&mut stream));
            let text = if i == 0 {
                "Premature answer"
            } else {
                "Verified final answer"
            };
            let body = json!({"choices":[{
                "message":{"role":"assistant","content":text},"finish_reason":"stop"
            }]})
            .to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        messages
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "base_url":format!("http://{address}/v1"),"model":"mock"}],
            "hooks":[{"event":"Stop","command":"echo 'Confirm tests' >&2; exit 2",
                "timeout_seconds":5}]
        })
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "run",
            "Do work",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Verified final answer"));
    assert!(requests[1].contains("Confirm tests"));
}

#[test]
fn sdk_mode_emits_json_lines_and_preserves_context() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let body = json!({"choices":[{"message":{"role":"assistant",
                "content":if step==0{"First reply"}else{"Second reply"}},"finish_reason":"stop"}]})
            .to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let conf = dir.path().join("config.json");
    fs::write(
        &conf,
        json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","base_url":format!("http://{addr}/v1"),
        "model":"fake"}]})
        .to_string(),
    )
    .unwrap();
    let mut process = binary()
        .args([
            "--workspace",
            dir.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "sdk",
        ])
        .env("ATLAS_CONFIG", &conf)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut writer = process.stdin.take().unwrap();
        writeln!(writer, "{}", json!({"type":"user","content":"First"})).unwrap();
        writeln!(writer, "{}", json!({"type":"user","content":"Second"})).unwrap();
    }
    let output = process.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(requests[1].contains("First reply"));
    let events = String::from_utf8(output.stdout).unwrap();
    let records: Vec<Value> = events
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(records.iter().filter(|v| v["type"] == "result").count(), 2);
    assert!(records.iter().any(|v| v["type"] == "system"));
    assert!(records.iter().any(|v| v["type"] == "assistant"));
}

#[test]
fn team_runs_two_members_with_separate_contexts_and_stable_result_order() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..4 {
            let (mut conn, _) = listener.accept().unwrap();
            let raw = read_request(&mut conn);
            let request: Value =
                serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let messages = request["messages"].as_array().unwrap();
            let serialized = serde_json::to_string(messages).unwrap();
            let payload = if step == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"team-1","type":"function","function":{
                        "name":"spawn_team","arguments":json!({"tasks":[
                            {"name":"one","prompt":"Analyze A"},
                            {"name":"two","prompt":"Analyze B"}
                        ]}).to_string()}}
                    ]},"finish_reason":"tool_calls"}]})
            } else if messages.last().is_some_and(|m| m["role"] == "tool") {
                json!({"choices":[{"message":{"role":"assistant","content":"Team complete"},
                    "finish_reason":"stop"}]})
            } else if serialized.contains("Analyze A") {
                json!({"choices":[{"message":{"role":"assistant","content":"Result A"},
                    "finish_reason":"stop"}]})
            } else if serialized.contains("Analyze B") {
                json!({"choices":[{"message":{"role":"assistant","content":"Result B"},
                    "finish_reason":"stop"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant","content":"Team complete"},
                    "finish_reason":"stop"}]})
            };
            let body = payload.to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            conn.flush().unwrap();
            requests.push(serialized);
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"mock",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Team tasks",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Team complete"));
    assert!(requests.iter().any(|q| q.contains("Analyze A")));
    assert!(requests.iter().any(|q| q.contains("Analyze B")));
    assert!(requests
        .iter()
        .any(|q| q.contains("Result A") && q.contains("Result B")));
    assert_eq!(
        fs::read_dir(root.path().join(".atlas/sessions"))
            .unwrap()
            .count(),
        3
    );
}

#[test]
fn sdk_streaming_emits_incremental_chunks_and_final_result() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        assert!(request.contains("\"stream\":true"));
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",sse.len(),sse).unwrap();
        stream.flush().unwrap();
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"mock","stream":true
        }]})
        .to_string(),
    )
    .unwrap();
    let mut process = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "sdk",
        ])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = process.stdin.take().unwrap();
        writeln!(input, "{}", json!({"type":"user","content":"Say hello"})).unwrap();
    }
    let output = process.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = String::from_utf8(output.stdout).unwrap();
    let events = result
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect::<Vec<_>>();
    let deltas = events
        .iter()
        .filter(|v| v["type"] == "assistant_delta")
        .filter_map(|v| v["delta"].as_str())
        .collect::<String>();
    assert_eq!(deltas, "Hello");
    assert!(events
        .iter()
        .any(|v| v["type"] == "result" && v["content"] == "Hello"));
}

#[cfg(unix)]
#[test]
fn background_shell_model_tool_roundtrip_runs_and_reads_real_process() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut log = Vec::new();
        for turn in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            let request: Value =
                serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let messages = request["messages"].as_array().unwrap();
            let reply = if turn == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"start1","type":"function","function":{
                        "name":"run_command","arguments":json!({
                            "command":"printf background-ready",
                            "run_in_background":true,"description":"Test shell"
                        }).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else if turn == 1 {
                let last: Value =
                    serde_json::from_str(messages.last().unwrap()["content"].as_str().unwrap())
                        .unwrap();
                let task_id = last["task_id"].as_str().unwrap().to_owned();
                log.push(last);
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"read1","type":"function","function":{
                        "name":"task_output","arguments":json!({
                            "task_id":task_id,"block":true,"timeout":5000
                        }).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else {
                let last: Value =
                    serde_json::from_str(messages.last().unwrap()["content"].as_str().unwrap())
                        .unwrap();
                log.push(last);
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Background shell verified."},"finish_reason":"stop"}]})
            };
            let body = reply.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        log
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"mock"
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Run a background test",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let logs = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Background shell verified."));
    assert_eq!(logs[0]["status"], "running");
    assert_eq!(logs[1]["retrieval_status"], "success");
    assert_eq!(logs[1]["task"]["status"], "completed");
    assert!(logs[1]["task"]["output"]
        .as_str()
        .unwrap()
        .contains("background-ready"));
}

#[test]
fn sdk_json_lines_never_contain_javascript_unicode_line_separators() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _request = read_request(&mut stream);
        let message = "first\u{2028}middle\u{2029}last";
        let body = json!({"choices":[{"message":{
            "role":"assistant","content":message
        },"finish_reason":"stop"}]})
        .to_string();
        write!(stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),body).unwrap();
        stream.flush().unwrap();
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"mock"
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "sdk",
        ])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({"type":"user","content":"Reply with separators"})
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8(output.stdout).unwrap();
    assert!(!wire.contains('\u{2028}'));
    assert!(!wire.contains('\u{2029}'));
    let events = wire
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let result = events.iter().find(|e| e["type"] == "result").unwrap();
    assert_eq!(result["content"], "first\u{2028}middle\u{2029}last");
}

#[test]
fn original_task_tools_end_to_end_model_calls_and_disk_graph() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut seen = Vec::<String>::new();
        let script = [
            (
                "TaskCreate",
                json!({"subject":"Implement","description":"Implement feature"}),
            ),
            (
                "TaskCreate",
                json!({"subject":"Verify","description":"Run integration tests"}),
            ),
            (
                "TaskUpdate",
                json!({"taskId":"2","addBlockedBy":["1"],"owner":"tester"}),
            ),
            ("TaskGet", json!({"taskId":"2"})),
            ("TaskList", json!({})),
            ("TaskUpdate", json!({"taskId":"1","status":"completed"})),
            ("TaskList", json!({})),
        ];
        for index in 0..=script.len() {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            let req: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            if let Some(last) = req["messages"].as_array().unwrap().last() {
                if last["role"] == "tool" {
                    seen.push(last["content"].as_str().unwrap().to_owned());
                }
            }
            let response = if let Some((name, args)) = script.get(index) {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":format!("task-{index}"),"type":"function",
                        "function":{"name":name,"arguments":args.to_string()}}]},
                    "finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Task workflow verified."},"finish_reason":"stop"}]})
            };
            let data = response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                data.len(),data).unwrap();
            stream.flush().unwrap();
        }
        seen
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"mock",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let response = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Create two tasks, link dependencies and finish",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let results = server.join().unwrap();
    assert!(
        response.status.success(),
        "{}",
        String::from_utf8_lossy(&response.stderr)
    );
    assert!(String::from_utf8_lossy(&response.stdout).contains("Task workflow verified."));
    assert_eq!(results.len(), 7);
    assert!(results[0].contains("Task #1 created successfully: Implement"));
    assert!(results[1].contains("Task #2 created successfully: Verify"));
    assert!(results[2].contains("blockedBy"));
    assert!(results[3].contains("Blocked by: #1"));
    assert!(results[4].contains("[blocked by #1]"));
    assert!(results[5].contains("status"));
    assert!(!results[6].contains("[blocked by #1]"));
    let list_dir = root.path().join(".atlas/task-lists");
    let entries = fs::read_dir(&list_dir).unwrap().collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let tasks = fs::read_dir(entries[0].as_ref().unwrap().path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .count();
    assert_eq!(tasks, 2);
}

#[test]
fn model_driven_team_create_message_receive_and_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let steps = [
            (
                "TeamCreate",
                json!({"team_name":"review","description":"Review code"}),
            ),
            (
                "SendMessage",
                json!({"to":"team-lead","summary":"Review note","message":"Check CI output"}),
            ),
            ("TeamInbox", json!({"mark_read":true})),
            ("TeamDelete", json!({})),
        ];
        let mut outputs = Vec::new();
        let mut saw_auto_delivery = false;
        for index in 0..=steps.len() {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let payload: Value =
                serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let messages = payload["messages"].as_array().unwrap();
            if messages.iter().any(|m| {
                m["role"] == "user"
                    && m["content"]
                        .as_str()
                        .unwrap_or("")
                        .contains("<teammate-message teammate_id=\"team-lead\"")
            }) {
                saw_auto_delivery = true;
            }
            if let Some(last) = messages.iter().rev().find(|m| m["role"] == "tool") {
                outputs.push(last["content"].as_str().unwrap().to_owned());
            }
            let reply = if let Some((name, args)) = steps.get(index) {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":format!("team-{index}"),"type":"function",
                        "function":{"name":name,"arguments":args.to_string()}}]},
                    "finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Team lifecycle complete."},"finish_reason":"stop"}]})
            };
            let body = reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        (outputs, saw_auto_delivery)
    });
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            dir.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Run team lifecycle",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let (results, saw_auto_delivery) = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Team lifecycle complete."));
    assert_eq!(results.len(), 4);
    assert!(results[0].contains("review"));
    assert!(results[1].contains("success"));
    assert!(
        saw_auto_delivery,
        "Teammate messages must reach the next model round"
    );
    assert_eq!(results[2], "[]");
    assert!(results[3].contains("Cleaned up team configuration"));
    assert!(!dir.path().join(".atlas/teams/review").exists());
}

#[test]
fn sdk_transient_mailbox_preserves_fifo_revision_and_validation() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let conf = root.path().join("config.json");
    fs::write(
        &conf,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible","model":"offline",
            "base_url":"http://127.0.0.1:9/v1"
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &conf)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let payloads = [
        json!({"type":"control","action":"mailbox_send","message":{
            "id":"1","source":"task","content":"first","timestamp":"now"}}),
        json!({"type":"control","action":"mailbox_send","message":{
            "id":"2","source":"teammate","content":"second","timestamp":"now","from":"peer"}}),
        json!({"type":"control","action":"mailbox_poll"}),
        json!({"type":"control","action":"mailbox_poll"}),
        json!({"type":"control","action":"mailbox_poll"}),
        json!({"type":"control","action":"mailbox_send","message":{"id":"bad"}}),
    ];
    {
        let mut stdin = child.stdin.take().unwrap();
        for item in &payloads {
            writeln!(stdin, "{item}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|v| v["type"] == "control_response")
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 6);
    assert_eq!(events[0]["revision"], 1);
    assert_eq!(events[1]["revision"], 2);
    assert_eq!(events[2]["message"]["content"], "first");
    assert_eq!(events[3]["message"]["content"], "second");
    assert_eq!(events[2]["revision"], 2);
    assert_eq!(events[4]["message"], Value::Null);
    assert_eq!(events[5]["ok"], false);
}

#[cfg(unix)]
#[test]
fn actual_model_tool_approval_obeys_explicit_allow_and_deny() {
    for (behavior, mode, expected_run) in [("allow", "ask", true), ("deny", "full", false)] {
        let dir = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut tool_feedback = String::new();
            for turn in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let raw = read_request(&mut stream);
                let input: Value =
                    serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
                let response = if turn == 0 {
                    json!({"choices":[{"message":{"role":"assistant","content":null,
                        "tool_calls":[{"id":"permission-test","type":"function","function":{
                            "name":"run_command",
                            "arguments":json!({"command":"echo EXECUTED > allowed-marker.txt"}).to_string()
                        }}]},"finish_reason":"tool_calls"}]})
                } else {
                    tool_feedback = input["messages"].as_array().unwrap().last().unwrap()
                        ["content"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    json!({"choices":[{"message":{"role":"assistant",
                        "content":"Permission contract checked."},"finish_reason":"stop"}]})
                };
                let body = response.to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),body).unwrap();
                stream.flush().unwrap();
            }
            tool_feedback
        });
        let config = dir.path().join("config.json");
        fs::write(
            &config,
            json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"test"
        }],"permission_rules":[{"behavior":behavior,
            "rule":"Bash(echo EXECUTED > allowed-marker.txt)"}]})
            .to_string(),
        )
        .unwrap();
        let output = binary()
            .args([
                "--workspace",
                dir.path().to_str().unwrap(),
                "--approval",
                mode,
                "run",
                "Try tool rule",
            ])
            .env("ATLAS_CONFIG", &config)
            .output()
            .unwrap();
        let tool_feedback = server.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Permission contract checked."));
        let marker = dir.path().join("allowed-marker.txt");
        assert_eq!(marker.exists(), expected_run, "policy behavior {behavior}");
        if expected_run {
            assert!(!tool_feedback.starts_with("ERROR:"), "{tool_feedback}");
        } else {
            assert!(
                tool_feedback.contains("denied by explicit permission rule"),
                "{tool_feedback}"
            );
        }
    }
}

#[test]
fn gemini_sse_sdk_model_tool_roundtrip_preserves_streaming_and_context() {
    use std::process::Stdio;
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("verify.txt"),
        "verified gemini streaming tool\n",
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut seen = Vec::new();
        for index in 0..2 {
            let (mut conn, _) = listener.accept().unwrap();
            let raw = read_request(&mut conn);
            assert!(
                raw.lines()
                    .next()
                    .unwrap_or("")
                    .contains(":streamGenerateContent?alt=sse"),
                "Expected streaming Gemini endpoint"
            );
            assert!(raw
                .to_ascii_lowercase()
                .contains("x-goog-api-key: test-gemini-key"));
            let body: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            seen.push(body);
            let events = if index == 0 {
                vec![json!({"candidates":[{"content":{"parts":[
                    {"text":"Checking "},
                    {"functionCall":{"name":"read_file","args":{"path":"verify.txt"}}}
                ]},"finishReason":"STOP"}],"usageMetadata":{
                    "promptTokenCount":6,"candidatesTokenCount":3,"totalTokenCount":9
                }})]
            } else {
                vec![
                    json!({"candidates":[{"content":{"parts":[{"text":"Verified "}]}}]}),
                    json!({"candidates":[{"content":{"parts":[{"text":"stream complete"}]},"finishReason":"STOP"}],
                        "usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":4,"totalTokenCount":14}}),
                ]
            };
            let sse = events
                .iter()
                .map(|x| format!("data: {}\r\n\r\n", x))
                .collect::<String>();
            write!(conn,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                sse.len(),sse).unwrap();
            conn.flush().unwrap();
        }
        seen
    });
    let config = workspace.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock-gemini",
            "providers":[{"id":"mock-gemini","kind":"gemini",
                "base_url":format!("http://{addr}/v1beta"),"model":"gemini-test",
                "api_key_env":"ATLAS_TEST_GEMINI_KEY","stream":true}]
        })
        .to_string(),
    )
    .unwrap();
    let mut process = binary()
        .args([
            "--workspace",
            workspace.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "sdk",
        ])
        .env("ATLAS_CONFIG", &config)
        .env("ATLAS_TEST_GEMINI_KEY", "test-gemini-key")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = process.stdin.take().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"type":"user","content":"Read verify.txt and summarize"})
        )
        .unwrap();
    }
    let output = process.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 2);
    assert!(requests[0]["tools"][0]["functionDeclarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["name"] == "read_file"));
    let second = requests[1]["contents"].as_array().unwrap();
    assert!(
        second.iter().any(|entry| entry["parts"]
            .as_array()
            .is_some_and(|parts| parts.iter().any(|p| p.get("functionResponse").is_some()))),
        "Tool result was not restored to the next Gemini request: {second:?}"
    );
    let lines = String::from_utf8(output.stdout).unwrap();
    let events = lines
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect::<Vec<_>>();
    let deltas = events
        .iter()
        .filter(|v| v["type"] == "assistant_delta")
        .filter_map(|v| v["delta"].as_str())
        .collect::<Vec<_>>();
    assert!(deltas.contains(&"Checking "));
    assert!(deltas.contains(&"Verified "));
    assert!(deltas.contains(&"stream complete"));
    assert_eq!(
        events.iter().find(|e| e["type"] == "result").unwrap()["content"],
        "Verified stream complete"
    );
}

#[test]
fn chat_permission_mode_cycle_controls_real_model_tool_mutations() {
    use std::process::Stdio;
    for (cycles, expected_allowed) in [(1usize, true), (2usize, false)] {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut tool_response = String::new();
            for turn in 0..2 {
                let (mut conn, _) = listener.accept().unwrap();
                let raw = read_request(&mut conn);
                let request: Value =
                    serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
                let payload = if turn == 0 {
                    json!({"choices":[{"message":{"role":"assistant",
                        "content":null,"tool_calls":[{
                            "id":"mode-write","type":"function",
                            "function":{"name":"write_file",
                                "arguments":json!({
                                    "path":"mode-write.txt","content":"written with mode"
                                }).to_string()}
                        }]},"finish_reason":"tool_calls"}]})
                } else {
                    tool_response = request["messages"].as_array().unwrap().last().unwrap()
                        ["content"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    json!({"choices":[{"message":{"role":"assistant",
                        "content":"Completed."},"finish_reason":"stop"}]})
                };
                let body = payload.to_string();
                write!(conn,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),body).unwrap();
                conn.flush().unwrap();
            }
            tool_response
        });
        let config = root.path().join("config.json");
        fs::write(
            &config,
            json!({"default_provider":"mock","providers":[{
                "id":"mock","kind":"openai-compatible","model":"test",
                "base_url":format!("http://{addr}/v1")
            }]})
            .to_string(),
        )
        .unwrap();
        let mut child = binary()
            .args([
                "--workspace",
                root.path().to_str().unwrap(),
                "--approval",
                "ask",
                "chat",
            ])
            .env("ATLAS_CONFIG", &config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            let mut input = child.stdin.take().unwrap();
            for _ in 0..cycles {
                writeln!(input, "/mode").unwrap();
            }
            writeln!(input, "Create mode-write.txt").unwrap();
            writeln!(input, "/exit").unwrap();
        }
        let output = child.wait_with_output().unwrap();
        let tool_result = server.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            root.path().join("mode-write.txt").exists(),
            expected_allowed
        );
        if expected_allowed {
            assert!(tool_result.contains("Wrote"), "{tool_result}");
        } else {
            assert!(
                tool_result.contains("denied by permission mode"),
                "{tool_result}"
            );
        }
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Completed."), "{stdout}");
    }
}

#[test]
fn sdk_control_request_recovers_legacy_ids_without_network() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    assert!(binary()
        .args(["init"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap()
        .status
        .success());
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for input in [
            json!({"type":"control_request","action":"permission_mode",
                "requestId":"legacy"}),
            json!({"type":"control","action":"tasks","request_id":"correct",
                "requestId":"old"}),
            json!({"type":"control_request","action":"unknown_action",
                "requestId":"invalid"}),
        ] {
            writeln!(stdin, "{input}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decoded = String::from_utf8(output.stdout).unwrap();
    let messages = decoded
        .lines()
        .map(|x| serde_json::from_str::<Value>(x).unwrap())
        .filter(|v| v["type"] == "control_response")
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["request_id"], "legacy");
    assert_eq!(messages[0]["ok"], true);
    assert_eq!(messages[1]["request_id"], "correct");
    assert_eq!(messages[1]["ok"], true);
    assert_eq!(messages[2]["request_id"], "invalid");
    assert_eq!(messages[2]["ok"], false);
}

#[test]
fn sdk_native_control_envelopes_switch_model_enforce_approval_and_preserve_ids() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    assert!(binary()
        .args(["init"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap()
        .status
        .success());
    let mut child = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "sdk",
        ])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for event in [
            json!({"type":"control_request","requestId":"m-1",
                "request":{"subtype":"set_model","model":"my-custom-model"}}),
            json!({"type":"control_request","request_id":"p-2",
                "request":{"subtype":"set_permission_mode","mode":"bypassPermissions"}}),
            json!({"type":"control_request","request_id":"p-3",
                "request":{"subtype":"set_permission_mode","mode":"plan"}}),
            json!({"type":"control_request","request_id":"t-4",
                "request":{"subtype":"stop_task","task_id":"invalid"}}),
            json!({"type":"control_request","request_id":"z-5",
                "request":{"subtype":"not_implemented"}}),
        ] {
            writeln!(stdin, "{event}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8(output.stdout).unwrap();
    let messages = wire
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|x| x["type"] == "control_response")
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 5);
    for (index, id) in ["m-1", "p-2", "p-3", "t-4", "z-5"].iter().enumerate() {
        assert_eq!(messages[index]["response"]["request_id"], *id);
    }
    assert_eq!(messages[0]["response"]["subtype"], "success");
    assert_eq!(
        messages[0]["response"]["response"]["model"],
        "my-custom-model"
    );
    assert_eq!(messages[1]["response"]["subtype"], "error");
    assert!(messages[1]["response"]["error"]
        .as_str()
        .unwrap()
        .contains("requires --approval full"));
    assert_eq!(messages[2]["response"]["subtype"], "success");
    assert_eq!(messages[2]["response"]["response"]["mode"], "plan");
    assert_eq!(messages[3]["response"]["subtype"], "error");
    assert_eq!(messages[4]["response"]["subtype"], "error");
}

#[test]
fn sdk_cost_ledger_survives_model_switch_and_session_resume() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..2 {
            let (mut connection, _) = listener.accept().unwrap();
            requests.push(read_request(&mut connection));
            let usage = if step == 0 {
                json!({"prompt_tokens":1_000_000,"completion_tokens":100_000,
                    "total_tokens":1_100_000})
            } else {
                json!({"prompt_tokens":500_000,"completion_tokens":100_000,
                    "total_tokens":600_000,
                    "prompt_tokens_details":{"cached_tokens":200_000}})
            };
            let reply = json!({"choices":[{"message":{"role":"assistant",
                "content":format!("Response {step}")},
                "finish_reason":"stop"}],"usage":usage})
            .to_string();
            write!(connection,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(), reply).unwrap();
            connection.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "base_url":format!("http://{address}/v1"),"model":"first"}],
            "model_prices":[
                {"provider":"mock","model":"first",
                    "input_per_million":1.0,"output_per_million":2.0},
                {"provider":"mock","model":"second",
                    "input_per_million":3.0,"output_per_million":4.0,
                    "cache_read_per_million":0.5}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for value in [
            json!({"type":"user","content":"first prompt"}),
            json!({"type":"control","action":"model","provider":"mock","model":"second"}),
            json!({"type":"user","content":"second prompt"}),
            json!({"type":"control","action":"cost","requestId":"c1"}),
        ] {
            writeln!(input, "{value}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(requests[0].contains("\"model\":\"first\""));
    assert!(requests[1].contains("\"model\":\"second\""));
    let events: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let cost = events.iter().find(|v| v["action"] == "cost").unwrap();
    assert_eq!(cost["request_id"], "c1");
    assert_eq!(cost["cost"]["models"].as_array().unwrap().len(), 2);
    let mut usd = 0.0;
    for model in cost["cost"]["models"].as_array().unwrap() {
        usd += model["known_cost_usd"].as_f64().unwrap();
        assert_eq!(model["has_unpriced_usage"], false);
    }
    // first: $1.2; second: uncached .3M*$3 + cached .2M*$0.5
    // plus output .1M*$4 = $1.4; total $2.6.
    assert!((usd - 2.6).abs() < 1e-9, "{cost}");
    let session_files = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(session_files.len(), 1);
    let stored: Value = serde_json::from_slice(&fs::read(&session_files[0]).unwrap()).unwrap();
    assert_eq!(stored["cost"]["models"].as_array().unwrap().len(), 2);
    assert_eq!(stored["usage"]["cache_read_input_tokens"], 200_000);
    let session_id = stored["id"].as_str().unwrap();
    let resumed = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "sdk",
            "--resume",
            session_id,
        ])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut resumed = resumed;
    writeln!(
        resumed.stdin.take().unwrap(),
        "{}",
        json!({"type":"control","action":"cost"})
    )
    .unwrap();
    let resumed_out = resumed.wait_with_output().unwrap();
    assert!(
        resumed_out.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed_out.stderr)
    );
    let resumed_json = String::from_utf8(resumed_out.stdout).unwrap();
    assert!(resumed_json.lines().any(|l| {
        let v: Value = serde_json::from_str(l).unwrap();
        v["action"] == "cost" && v["cost"]["models"].as_array().is_some_and(|a| a.len() == 2)
    }));
}

#[test]
fn real_model_tool_edits_update_persisted_line_and_cost_metrics() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for step in 0..3 {
            let (mut conn, _) = listener.accept().unwrap();
            let _ = read_request(&mut conn);
            let message = match step {
                0 => json!({"role":"assistant","content":null,"tool_calls":[{
                    "id":"write-1","type":"function",
                    "function":{"name":"write_file",
                        "arguments":json!({"path":"tracked.txt","content":"alpha\nbeta\n"}).to_string()}
                }]}),
                1 => json!({"role":"assistant","content":null,"tool_calls":[{
                    "id":"edit-1","type":"function",
                    "function":{"name":"edit_file",
                        "arguments":json!({"path":"tracked.txt","old":"beta","new":"gamma"}).to_string()}
                }]}),
                _ => json!({"role":"assistant","content":"Changes verified"}),
            };
            let reply = json!({"choices":[{"message":message,
                "finish_reason":if step<2{"tool_calls"}else{"stop"}}],
                "usage":{"prompt_tokens":20,"completion_tokens":10,"total_tokens":30}})
            .to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),reply).unwrap();
            conn.flush().unwrap();
        }
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"test",
        "base_url":format!("http://{address}/v1")
    }],"model_prices":[{"provider":"mock","model":"test",
        "input_per_million":1.0,"output_per_million":2.0}]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Create and edit a file",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.path().join("tracked.txt")).unwrap(),
        "alpha\ngamma\n"
    );
    let dir = root.path().join(".atlas/sessions");
    let files = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let session: Value = serde_json::from_slice(&fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(session["usage"]["input_tokens"], 60);
    assert_eq!(session["usage"]["output_tokens"], 30);
    assert_eq!(session["cost"]["total_lines_added"], 4);
    assert_eq!(session["cost"]["total_lines_removed"], 1);
    assert_eq!(session["cost"]["models"][0]["has_unpriced_usage"], false);
    let usd = session["cost"]["models"][0]["known_cost_usd"]
        .as_f64()
        .unwrap();
    assert!((usd - 0.00012).abs() < 1e-12, "{usd}");
}

#[test]
fn failed_team_member_usage_is_still_charged_to_parent_session() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..3 {
            let (mut conn, _) = listener.accept().unwrap();
            let raw = read_request(&mut conn);
            let request: Value =
                serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            requests.push(request);
            let payload = match step {
                0 => json!({"choices":[{"message":{"role":"assistant",
                    "content":null,"tool_calls":[{
                        "id":"spawn-failed","type":"function",
                        "function":{"name":"spawn_team",
                            "arguments":json!({"tasks":[{
                                "name":"failed","prompt":"Failed worker prompt"
                            }]}).to_string()}
                    }]},"finish_reason":"tool_calls"}],
                    "usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}),
                1 => json!({"choices":[{"message":{"role":"assistant",
                        "content":"Partial work","tool_calls":null},
                    "finish_reason":"length"}],
                    "usage":{"prompt_tokens":50,"completion_tokens":5,"total_tokens":55}}),
                _ => json!({"choices":[{"message":{"role":"assistant",
                        "content":"Team failure acknowledged"},
                    "finish_reason":"stop"}],
                    "usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}),
            };
            let body = payload.to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            conn.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"model",
        "base_url":format!("http://{address}/v1")
    }],"model_prices":[{"provider":"mock","model":"model",
        "input_per_million":1.0,"output_per_million":2.0}]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Start failing team",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tool_result = requests[2]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(tool_result.contains("\"success\":false"), "{tool_result}");
    assert!(tool_result.contains("output limit"), "{tool_result}");
    let sessions = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .map(|entry| {
            serde_json::from_slice::<Value>(&fs::read(entry.unwrap().path()).unwrap()).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(sessions.len(), 2);
    let parent = sessions
        .iter()
        .find(|session| {
            session["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .contains("Start failing team")
        })
        .unwrap();
    let child = sessions
        .iter()
        .find(|session| {
            session["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .contains("Failed worker prompt")
        })
        .unwrap();
    assert_eq!(child["usage"]["input_tokens"], 50);
    assert_eq!(parent["usage"]["input_tokens"], 70);
    assert_eq!(parent["usage"]["output_tokens"], 9);
    assert_eq!(parent["cost"]["models"][0]["usage"]["input_tokens"], 70);
    let usd = parent["cost"]["models"][0]["known_cost_usd"]
        .as_f64()
        .unwrap();
    assert!((usd - 0.000088).abs() < 1e-12, "{usd}");
}

#[test]
fn sdk_queue_drain_honors_priority_fifo_and_worker_isolation() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut conn, _) = listener.accept().unwrap();
            let raw = read_request(&mut conn);
            requests.push(raw.clone());
            let content = format!("Reply {index}");
            let response = json!({"choices":[{"message":{
                "role":"assistant","content":content},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}})
            .to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),response).unwrap();
            conn.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for value in [
            json!({"type":"user","content":"NEXT_REQUEST","queued":true}),
            json!({"type":"user","content":"LATER_REQUEST","queued":true,"priority":"later"}),
            json!({"type":"user","content":"NOW_REQUEST","queued":true,"priority":"now","workload":"urgent"}),
            json!({"type":"user","content":"WORKER_REQUEST","queued":true,
                "priority":"now","agentId":"worker"}),
            json!({"type":"control","action":"queue_drain","request_id":"drain",
                "max_priority":"next"}),
            json!({"type":"control","action":"queue_snapshot","request_id":"snapshot"}),
        ] {
            writeln!(input, "{value}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(requests[0].contains("NOW_REQUEST"));
    assert!(!requests[0].contains("NEXT_REQUEST"));
    assert!(requests[1].contains("NEXT_REQUEST"));
    assert!(!requests[1].contains("WORKER_REQUEST"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let drain = events.iter().find(|e| e["request_id"] == "drain").unwrap();
    assert_eq!(drain["ok"], true);
    assert_eq!(drain["result"]["results"].as_array().unwrap().len(), 2);
    assert_eq!(drain["result"]["usage"]["input_tokens"], 20);
    let snapshot = events
        .iter()
        .find(|e| e["request_id"] == "snapshot")
        .unwrap();
    assert_eq!(snapshot["result"]["length"], 2);
    assert_eq!(snapshot["result"]["commands"][0]["value"], "LATER_REQUEST");
    assert_eq!(snapshot["result"]["commands"][1]["agentId"], "worker");
}

#[test]
fn sdk_queue_pop_editable_does_not_leak_system_notifications_into_input() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible","model":"offline",
            "base_url":"http://127.0.0.1:9/v1"
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for value in [
            json!({"type":"control","action":"queue_enqueue","command":{
                "mode":"prompt","value":"🌐 queued",
                "pastedContents":{"7":{"id":7,"type":"image","content":"YWJj"}}
            }}),
            json!({"type":"control","action":"queue_enqueue","notification":true,
                "command":{"mode":"task-notification","value":"<task-hidden/>"}}),
            json!({"type":"control","action":"queue_enqueue","command":{
                "mode":"prompt","value":"<meta-hidden/>","isMeta":true}}),
            json!({"type":"control","action":"queue_pop_editable",
                "request_id":"pop","current_input":"draft","cursor_offset":2}),
            json!({"type":"control","action":"queue_snapshot","request_id":"after"}),
        ] {
            writeln!(input, "{value}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let pop = events.iter().find(|v| v["request_id"] == "pop").unwrap();
    assert_eq!(pop["ok"], true);
    assert_eq!(pop["result"]["editable"]["text"], "🌐 queued\ndraft");
    assert_eq!(pop["result"]["editable"]["images"][0]["id"], 7);
    assert_eq!(pop["result"]["editable"]["cursor_offset"], 12);
    let rest = events.iter().find(|v| v["request_id"] == "after").unwrap();
    assert_eq!(rest["result"]["length"], 2);
    assert_eq!(rest["result"]["commands"][0]["mode"], "task-notification");
}

#[test]
fn model_invokes_skill_with_quote_aware_named_arguments_in_real_tool_loop() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(".atlas/skills/audit");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\narguments: file purpose\n---\nAudit $file for $purpose\nRaw: $ARGUMENTS\nIndexed: $1",
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut seen_result = String::new();
        for turn in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            let req: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            if turn == 1 {
                seen_result = req["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rev()
                    .find(|m| m["role"] == "tool")
                    .unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .to_owned();
            }
            let reply = if turn == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"skill-args","type":"function",
                        "function":{"name":"read_skill","arguments":json!({
                            "name":"audit","arguments":"src/lib.rs 'safety and reliability'"
                        }).to_string()}
                    }]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Skill context received."},"finish_reason":"stop"}]})
            };
            let response = reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),response).unwrap();
            stream.flush().unwrap();
        }
        seen_result
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"mock",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Load audit skill and proceed",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let result = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Skill context received."));
    assert!(
        result.contains("Audit src/lib.rs for safety and reliability"),
        "{result}"
    );
    assert!(
        result.contains("Indexed: safety and reliability"),
        "{result}"
    );
    assert!(
        result.contains("Raw: src/lib.rs 'safety and reliability'"),
        "{result}"
    );
}

#[test]
fn sdk_queue_drain_preserves_results_before_blocked_item() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let request = read_request(&mut conn);
        let response = json!({"choices":[{"message":{"role":"assistant",
            "content":"First completed"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}})
        .to_string();
        write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),response).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for value in [
            json!({"type":"control","action":"queue_enqueue","command":{
                "mode":"prompt","value":"SAFE_PROMPT","priority":"now"}}),
            json!({"type":"control","action":"queue_enqueue","command":{
                "mode":"bash","value":"echo do-not-run","priority":"next"}}),
            json!({"type":"control","action":"queue_drain","request_id":"drain"}),
            json!({"type":"control","action":"queue_snapshot","request_id":"after"}),
        ] {
            writeln!(input, "{value}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(request.contains("SAFE_PROMPT"));
    assert!(!request.contains("do-not-run"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let drain = events.iter().find(|v| v["request_id"] == "drain").unwrap();
    assert_eq!(drain["ok"], true, "{drain}");
    assert_eq!(drain["result"]["results"].as_array().unwrap().len(), 1);
    assert_eq!(drain["result"]["results"][0]["content"], "First completed");
    assert_eq!(drain["result"]["blocked"]["reason"], "bash");
    let after = events.iter().find(|v| v["request_id"] == "after").unwrap();
    assert_eq!(after["result"]["length"], 1);
    assert_eq!(after["result"]["commands"][0]["value"], "echo do-not-run");
    assert_eq!(
        drain["result"]["blocked"]["identity"],
        after["result"]["commands"][0]["identity"]
    );
}

#[test]
fn sdk_invalid_queue_input_and_slash_routing_keep_session_alive() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible","model":"offline",
            "base_url":"http://127.0.0.1:9/v1"
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for value in [
            json!({"type":"user","content":"INVALID","queued":true,"priority":"urgent"}),
            json!({"type":"user","content":"/model other","queued":true,"priority":"now"}),
            json!({"type":"control","action":"queue_drain","request_id":"drain"}),
            json!({"type":"control","action":"queue_snapshot","request_id":"after"}),
        ] {
            writeln!(input, "{value}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(events.iter().any(|v| v["type"] == "error"
        && v["message"]
            .as_str()
            .unwrap_or("")
            .contains("Invalid queued user command")));
    let drain = events.iter().find(|v| v["request_id"] == "drain").unwrap();
    assert_eq!(drain["ok"], false);
    assert!(drain["error"].as_str().unwrap().contains("slash-command"));
    let after = events.iter().find(|v| v["request_id"] == "after").unwrap();
    assert_eq!(after["ok"], true);
    assert_eq!(after["result"]["length"], 1);
    assert_eq!(after["result"]["commands"][0]["value"], "/model other");
}

#[test]
fn sdk_headless_queue_batches_consecutive_prompts_by_workload_and_meta() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let request = read_request(&mut conn);
        let response = json!({"choices":[{"message":{"role":"assistant",
            "content":"Handled combined request"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":40,"completion_tokens":8,"total_tokens":48}})
        .to_string();
        write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),response).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"mock",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for event in [
            json!({"type":"user","content":"AUDIT_FIRST","queued":true,"workload":"review"}),
            json!({"type":"user","content":"AUDIT_SECOND","queued":true,"workload":"review"}),
            json!({"type":"control","action":"queue_drain","request_id":"batch"}),
        ] {
            writeln!(input, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(request.contains("AUDIT_FIRST\\nAUDIT_SECOND"), "{request}");
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let batch = events.iter().find(|v| v["request_id"] == "batch").unwrap();
    assert_eq!(batch["ok"], true);
    assert_eq!(batch["result"]["results"].as_array().unwrap().len(), 2);
    assert_eq!(
        batch["result"]["results"][0]["content"],
        "Handled combined request"
    );
    assert_eq!(
        batch["result"]["results"][1]["content"],
        "Handled combined request"
    );
    // The one model round-trip is billed once, not twice for each queued ID.
    assert_eq!(batch["result"]["usage"]["input_tokens"], 40);
    assert_eq!(batch["result"]["length"], 0);
}

#[test]
fn sdk_queue_emits_structured_task_notification_before_model_result() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let request = read_request(&mut conn);
        let response = json!({"choices":[{"message":{"role":"assistant",
            "content":"Task inspected"},"finish_reason":"stop"}]})
        .to_string();
        write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),response).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = sdk.stdin.take().unwrap();
        for event in [
            json!({"type":"control","action":"queue_enqueue","notification":true,
            "command":{"mode":"task-notification",
                "value":"<task-id>job7</task-id><tool-use-id>call9</tool-use-id><status>killed</status><output-file>/tmp/out</output-file><summary>Stopped by user</summary><usage><total_tokens>45</total_tokens><tool_uses>3</tool_uses><duration_ms>220</duration_ms></usage>"
            }}),
            json!({"type":"control","action":"queue_drain","request_id":"done"}),
        ] {
            writeln!(stdin, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(request.contains("job7"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect::<Vec<_>>();
    let notification_pos = events
        .iter()
        .position(|v| v["type"] == "system" && v["subtype"] == "task_notification")
        .unwrap();
    let result_pos = events
        .iter()
        .position(|v| v["request_id"] == "done")
        .unwrap();
    assert!(notification_pos < result_pos);
    let notice = &events[notification_pos];
    assert_eq!(notice["task_id"], "job7");
    assert_eq!(notice["tool_use_id"], "call9");
    assert_eq!(notice["status"], "stopped");
    assert_eq!(notice["usage"]["total_tokens"], 45);
    assert_eq!(notice["usage"]["tool_uses"], 3);
    assert_eq!(notice["usage"]["duration_ms"], 220);
    assert_eq!(events[result_pos]["ok"], true);
}

#[test]
fn sdk_failed_model_turn_stops_drain_without_consuming_following_work() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let request = read_request(&mut conn);
        let body = r#"{"error":{"message":"model unavailable"}}"#;
        write!(conn,"HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),body).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for event in [
            json!({"type":"user","content":"FAIL_ME","queued":true,"workload":"one"}),
            json!({"type":"user","content":"DO_NOT_SEND","queued":true,"workload":"two"}),
            json!({"type":"control","action":"queue_drain","request_id":"failed"}),
            json!({"type":"control","action":"queue_snapshot","request_id":"remaining"}),
        ] {
            writeln!(input, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(request.contains("FAIL_ME"));
    assert!(!request.contains("DO_NOT_SEND"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let drain = events.iter().find(|v| v["request_id"] == "failed").unwrap();
    assert_eq!(drain["ok"], true, "{drain}");
    assert_eq!(drain["result"]["results"].as_array().unwrap().len(), 1);
    assert_eq!(drain["result"]["results"][0]["ok"], false);
    assert_eq!(drain["result"]["halted"]["reason"], "model-error");
    assert_eq!(drain["result"]["halted"]["identities"], json!([1]));
    let remaining = events
        .iter()
        .find(|v| v["request_id"] == "remaining")
        .unwrap();
    assert_eq!(remaining["result"]["length"], 1);
    assert_eq!(remaining["result"]["commands"][0]["value"], "DO_NOT_SEND");
}

#[test]
fn sdk_unsupported_content_blocks_are_preserved_until_explicitly_removed() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let request = read_request(&mut conn);
        let body = json!({"choices":[{"message":{"role":"assistant",
            "content":"Text blocks accepted"},"finish_reason":"stop"}]})
        .to_string();
        write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),body).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for event in [
            json!({"type":"user","queued":true,"content":[
                {"type":"text","text":"SAFE"},
                {"type":"document","source":{"type":"text","data":"MUST_NOT_DROP"}}]}),
            json!({"type":"control","action":"queue_drain","request_id":"blocked"}),
            json!({"type":"control","action":"queue_snapshot","request_id":"preserved"}),
            json!({"type":"control","action":"queue_remove_identity","identities":[1]}),
            json!({"type":"control","action":"queue_enqueue","command":{
                "mode":"prompt","value":[{"type":"text","text":"first"},
                    {"type":"text","text":"second"}]}}),
            json!({"type":"control","action":"queue_drain","request_id":"accepted"}),
        ] {
            writeln!(input, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(request.contains("\"text\":\"first\""), "{request}");
    assert!(request.contains("\"text\":\"second\""), "{request}");
    assert!(!request.contains("MUST_NOT_DROP"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let blocked = events
        .iter()
        .find(|v| v["request_id"] == "blocked")
        .unwrap();
    assert_eq!(blocked["ok"], false);
    assert!(blocked["error"]
        .as_str()
        .unwrap()
        .contains("unsupported-content"));
    let preserved = events
        .iter()
        .find(|v| v["request_id"] == "preserved")
        .unwrap();
    assert_eq!(preserved["result"]["length"], 1);
    assert_eq!(
        preserved["result"]["commands"][0]["value"][1]["type"],
        "document"
    );
    let accepted = events
        .iter()
        .find(|v| v["request_id"] == "accepted")
        .unwrap();
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(
        accepted["result"]["results"][0]["content"],
        "Text blocks accepted"
    );
}

#[test]
fn sdk_direct_structured_text_is_delivered_and_nontext_is_rejected_without_http() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let request = read_request(&mut conn);
        let body = json!({"choices":[{"message":{"role":"assistant",
            "content":"Direct text accepted"},"finish_reason":"stop"}]})
        .to_string();
        write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),body).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for event in [
            json!({"type":"user","content":[{"type":"text","text":"hello"},
                {"type":"document","source":{"type":"text","data":"PRIVATE"}}]}),
            json!({"type":"user","uuid":"direct-replay-1",
                "replayUserMessages":true,"message":{"content":[
                {"type":"text","text":"first"},{"type":"text","text":"second"}]}}),
        ] {
            writeln!(input, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(request.contains("\"text\":\"first\""), "{request}");
    assert!(request.contains("\"text\":\"second\""), "{request}");
    assert!(!request.contains("PRIVATE"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(events.iter().any(|v| v["type"] == "error"
        && v["message"]
            .as_str()
            .unwrap_or("")
            .contains("Unsupported user content")));
    let replay = events
        .iter()
        .find(|v| v["type"] == "user" && v["isReplay"] == true)
        .unwrap();
    assert_eq!(replay["uuid"], "direct-replay-1");
    assert_eq!(
        replay["message"]["content"],
        json!([
        {"type":"text","text":"first"},{"type":"text","text":"second"}])
    );
    let replay_index = events
        .iter()
        .position(|v| v["uuid"] == "direct-replay-1")
        .unwrap();
    let result_index = events
        .iter()
        .position(|v| v["type"] == "result" && v["content"] == "Direct text accepted")
        .unwrap();
    assert!(replay_index < result_index);
}

#[test]
fn sdk_queued_uuid_replays_survive_prompt_batching_and_preserve_source_content() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let response = json!({"choices":[{"message":{"role":"assistant",
                "content":if index==0 {"First batch"} else {"Second batch"}},
                "finish_reason":"stop"}]})
            .to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),response).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = sdk.stdin.take().unwrap();
        for event in [
            json!({"type":"user","queued":true,"content":"INVALID_UUID","uuid":42}),
            json!({"type":"user","queued":true,"content":"FIRST","uuid":"u-first",
                "workload":"review","origin":{"kind":"sdk"}}),
            json!({"type":"user","queued":true,"content":[
                {"type":"text","text":"SECOND"}],"uuid":"u-second","workload":"review"}),
            json!({"type":"user","queued":true,"content":"THIRD","uuid":"u-third",
                "workload":"other"}),
            json!({"type":"control","action":"queue_snapshot","request_id":"before"}),
            json!({"type":"control","action":"queue_drain","request_id":"drain",
                "replay_user_messages":true}),
            json!({"type":"control","action":"queue_snapshot","request_id":"after"}),
        ] {
            writeln!(input, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(requests[0].contains("FIRST") && requests[0].contains("SECOND"));
    // The second HTTP request legitimately contains the first turn in
    // conversation history. Verify its NEW user turn rather than asserting
    // the prior batch disappears from context.
    let second_body = requests[1].split_once("\r\n\r\n").unwrap().1;
    let second_json: Value = serde_json::from_str(second_body).unwrap();
    let last_user = second_json["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|message| message["role"] == "user")
        .unwrap();
    assert_eq!(last_user["content"], "THIRD");
    assert!(!requests[0].contains("INVALID_UUID"));
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(events.iter().any(|v| v["type"] == "error"
        && v["message"]
            .as_str()
            .unwrap_or("")
            .contains("Invalid queued user command")));
    let before = events.iter().find(|v| v["request_id"] == "before").unwrap();
    assert_eq!(before["result"]["length"], 3);
    assert_eq!(before["result"]["commands"][0]["uuid"], "u-first");
    assert_eq!(before["result"]["commands"][0]["origin"]["kind"], "sdk");
    assert_eq!(before["result"]["commands"][1]["uuid"], "u-second");
    let replays = events
        .iter()
        .filter(|v| v["type"] == "user" && v["isReplay"] == true)
        .collect::<Vec<_>>();
    assert_eq!(replays.len(), 3, "{events:?}");
    assert_eq!(
        replays
            .iter()
            .map(|v| v["uuid"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["u-first", "u-second", "u-third"]
    );
    assert_eq!(replays[0]["message"]["content"], "FIRST");
    assert_eq!(
        replays[1]["message"]["content"],
        json!([{"type":"text","text":"SECOND"}])
    );
    assert_eq!(replays[2]["message"]["content"], "THIRD");
    assert_eq!(replays[0]["parent_tool_use_id"], Value::Null);
    assert_eq!(replays[0]["session_id"], replays[1]["session_id"]);
    let drain_index = events
        .iter()
        .position(|v| v["request_id"] == "drain")
        .unwrap();
    assert!(events
        .iter()
        .enumerate()
        .filter(|(_, v)| v["isReplay"] == true)
        .all(|(index, _)| index < drain_index));
    let drain = &events[drain_index];
    assert_eq!(drain["ok"], true, "{drain}");
    assert_eq!(drain["result"]["results"].as_array().unwrap().len(), 3);
    assert_eq!(drain["result"]["results"][0]["content"], "First batch");
    assert_eq!(drain["result"]["results"][1]["content"], "First batch");
    assert_eq!(drain["result"]["results"][2]["content"], "Second batch");
    let after = events.iter().find(|v| v["request_id"] == "after").unwrap();
    assert_eq!(after["result"]["length"], 0);
}

#[test]
fn sdk_native_control_cancels_pending_uuid_without_model_side_effects() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        let response = json!({"choices":[{"message":{"role":"assistant",
            "content":"Surviving message processed"},"finish_reason":"stop"}]})
        .to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),response).unwrap();
        stream.flush().unwrap();
        request
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        for event in [
            json!({"type":"control_request","request_id":"init",
                "request":{"subtype":"initialize"}}),
            json!({"type":"user","content":"CANCEL_FIRST","queued":true,"uuid":"drop"}),
            json!({"type":"user","content":"CANCEL_SECOND","queued":true,"uuid":"drop"}),
            json!({"type":"user","content":"KEEP_ONLY","queued":true,"uuid":"keep"}),
            json!({"type":"control_request","requestId":"cancel-1",
                "request":{"subtype":"cancel_async_message","message_uuid":"drop"}}),
            json!({"type":"control_request","request_id":"cancel-2",
                "request":{"subtype":"cancel_async_message","message_uuid":"drop"}}),
            json!({"type":"control_request","request_id":"bad",
                "request":{"subtype":"cancel_async_message","message_uuid":7}}),
            json!({"type":"control_request","request_id":"mcp",
                "request":{"subtype":"mcp_status"}}),
            json!({"type":"control","action":"queue_snapshot","request_id":"before"}),
            json!({"type":"control","action":"queue_drain","request_id":"run",
                "replay_user_messages":true}),
            json!({"type":"control","action":"queue_snapshot","request_id":"after"}),
        ] {
            writeln!(input, "{event}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    let actual_request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(actual_request.contains("KEEP_ONLY"), "{actual_request}");
    assert!(!actual_request.contains("CANCEL_FIRST"), "{actual_request}");
    assert!(
        !actual_request.contains("CANCEL_SECOND"),
        "{actual_request}"
    );
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let nested = |id: &str| {
        events
            .iter()
            .find(|v| v["type"] == "control_response" && v["response"]["request_id"] == id)
            .unwrap()
    };
    assert_eq!(
        nested("cancel-1")["response"]["response"]["cancelled"],
        true
    );
    assert_eq!(
        nested("cancel-2")["response"]["response"]["cancelled"],
        false
    );
    assert_eq!(nested("bad")["response"]["subtype"], "error");
    assert_eq!(nested("init")["response"]["subtype"], "success");
    assert_eq!(nested("init")["response"]["response"]["account"], json!({}));
    assert!(nested("init")["response"]["response"]["models"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["value"] == "model"));
    assert_eq!(
        nested("mcp")["response"]["response"]["mcpServers"],
        json!([])
    );
    let legacy = |id: &str| events.iter().find(|v| v["request_id"] == id).unwrap();
    assert_eq!(legacy("before")["result"]["length"], 1);
    assert_eq!(legacy("before")["result"]["commands"][0]["uuid"], "keep");
    assert_eq!(legacy("after")["result"]["length"], 0);
    assert_eq!(legacy("run")["ok"], true);
    let replays = events
        .iter()
        .filter(|v| v["type"] == "user" && v["isReplay"] == true)
        .collect::<Vec<_>>();
    assert_eq!(replays.len(), 1);
    assert_eq!(replays[0]["uuid"], "keep");
}

#[cfg(unix)]
#[test]
fn sdk_mcp_status_reads_live_stdio_discovery_and_never_exposes_secrets() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    let script = r#"while IFS= read -r line; do
case "$line" in
  *'"method":"initialize"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"live-mcp","version":"1.2"}}}'
    ;;
  *'"method":"tools/list"'*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"read_entry","description":"Read entry","inputSchema":{"type":"object"}}]}}'
    ;;
esac
done"#;
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible","model":"local",
            "base_url":"http://127.0.0.1:11434/v1"
        }],"mcp_servers":[{
            "id":"local-mcp","command":"sh","args":["-c",script],
            "env":{"SECRET_VALUE":"must-not-leak"}
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        for id in ["one", "two"] {
            writeln!(
                input,
                "{}",
                json!({"type":"control_request","request_id":id,
                "request":{"subtype":"mcp_status"}})
            )
            .unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8(output.stdout).unwrap();
    assert!(!wire.contains("must-not-leak"));
    let events = wire
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|v| v["type"] == "control_response")
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    for (index, id) in ["one", "two"].iter().enumerate() {
        assert_eq!(events[index]["response"]["request_id"], *id);
        let server = &events[index]["response"]["response"]["mcpServers"][0];
        assert_eq!(server["name"], "local-mcp");
        assert_eq!(server["status"], "connected");
        assert_eq!(server["serverInfo"]["name"], "live-mcp");
        assert_eq!(server["tools"][0]["name"], "read_entry");
        assert_eq!(server["config"]["type"], "stdio");
        assert!(server["config"].get("env").is_none());
    }
}

#[test]
fn agent_invalid_tool_json_preserves_sibling_calls_and_model_followup() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut conn, _) = listener.accept().unwrap();
            requests.push(read_request(&mut conn));
            let response = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[
                        {"id":"bad-json","type":"function","function":{
                            "name":"read_file","arguments":"{bad-json"}},
                        {"id":"bad-shape","type":"function","function":{
                            "name":"write_file","arguments":"[1,2,3]"}},
                        {"id":"good-write","type":"function","function":{
                            "name":"write_file",
                            "arguments":json!({"path":"survived.txt",
                                "content":"persisted despite sibling failures\n"}).to_string()}}
                    ]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"All tool calls acknowledged"},"finish_reason":"stop"}]})
            };
            let body = response.to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            conn.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Make changes",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("All tool calls acknowledged"));
    assert_eq!(requests.len(), 2);
    let response_payload = &requests[1];
    assert!(response_payload.contains("bad-json"), "{response_payload}");
    assert!(response_payload.contains("bad-shape"), "{response_payload}");
    assert!(
        response_payload.contains("good-write"),
        "{response_payload}"
    );
    assert!(
        response_payload.contains("Invalid tool arguments JSON"),
        "{response_payload}"
    );
    assert!(
        response_payload.contains("Tool arguments must be a JSON object"),
        "{response_payload}"
    );
    assert!(root.path().join("survived.txt").is_file());
    let result = fs::read_to_string(root.path().join("survived.txt")).unwrap();
    assert!(result.contains("persisted despite sibling failures"));
    assert!(!root.path().join("bad-json").exists());
    let sessions = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(sessions.len(), 1);
    let history = fs::read_to_string(&sessions[0]).unwrap();
    for id in ["bad-json", "bad-shape", "good-write"] {
        assert!(history.contains(id), "missing persisted tool id {id}");
    }
}

#[cfg(unix)]
#[test]
fn agent_nonblocking_hook_exit_codes_preserve_tool_results_and_report_warnings() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("secret.txt"),
        "SECRET_DO_NOT_LEAK_PREHOOK\n",
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut conn, _) = listener.accept().unwrap();
            requests.push(read_request(&mut conn));
            let response = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[
                        {"id":"pre-failure","type":"function","function":{
                            "name":"read_file",
                            "arguments":json!({"path":"secret.txt"}).to_string()}},
                        {"id":"post-failure","type":"function","function":{
                            "name":"write_file",
                            "arguments":json!({"path":"hooked.txt",
                                "content":"write took effect"}).to_string()}},
                        {"id":"sibling-continues","type":"function","function":{
                            "name":"list_files",
                            "arguments":json!({"path":"."}).to_string()}}
                    ]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Hooks and sibling recovery confirmed"},"finish_reason":"stop"}]})
            };
            let body = response.to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            conn.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"model",
            "base_url":format!("http://{address}/v1")
        }],"hooks":[
            {"event":"PreToolUse","matcher":"^read_file$",
                "command":"echo 'pre-hook boom' >&2; exit 7","timeout_seconds":5},
            {"event":"PostToolUse","matcher":"^write_file$",
                "command":"echo 'post-hook boom' >&2; exit 9","timeout_seconds":5}
        ]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Check tool hooks",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Hooks and sibling recovery confirmed")
    );
    assert_eq!(requests.len(), 2);
    let followup = &requests[1];
    assert!(followup.contains("pre-failure"), "{followup}");
    assert!(followup.contains("post-failure"), "{followup}");
    assert!(followup.contains("sibling-continues"), "{followup}");
    // Original hooks.ts:2670-2695: exit codes other than 2 are visible
    // non-blocking errors. The tools still execute and their real outputs
    // reach the next model request; hook failures are reported separately.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("pre-hook boom"), "{stderr}");
    assert!(stderr.contains("post-hook boom"), "{stderr}");
    assert!(!followup.contains("PreToolUse hook failed"), "{followup}");
    assert!(
        !followup.contains("PostToolUse hook failed after tool execution"),
        "{followup}"
    );
    assert!(followup.contains("write took effect"), "{followup}");
    assert!(
        followup.contains("SECRET_DO_NOT_LEAK_PREHOOK"),
        "{followup}"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("hooked.txt")).unwrap(),
        "write took effect"
    );
}

#[cfg(unix)]
#[test]
fn sdk_mcp_resources_templates_and_prompts_roundtrip_preserve_structured_content() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("mcp-config.json");
    // Explicit IDs make the mock verify real JSON-RPC sequencing, not an
    // isolated unit helper. Two resource pages must retain the original order.
    let script = r#"while IFS= read -r line; do
case "$line" in
  *'"id":1'*)
    echo '{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"catalog","version":"1"},"capabilities":{"resources":{},"prompts":{},"tools":{}}}}' ;;
  *'"id":2'*)
    echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}' ;;
  *'"id":3'*)
    echo '{"jsonrpc":"2.0","id":3,"result":{"resources":[{"uri":"file:///one","name":"one"}],"nextCursor":"page-2"}}' ;;
  *'"id":4'*)
    echo '{"jsonrpc":"2.0","id":4,"result":{"resources":[{"uri":"file:///two","name":"two"}]}}' ;;
  *'"id":5'*)
    echo '{"jsonrpc":"2.0","id":5,"result":{"prompts":[{"name":"inspect","arguments":[{"name":"goal","required":true}]}]}}' ;;
  *'"id":6'*)
    echo '{"jsonrpc":"2.0","id":6,"result":{"resourceTemplates":[{"uriTemplate":"file:///{path}","name":"files"}]}}' ;;
  *'"id":7'*)
    echo '{"jsonrpc":"2.0","id":7,"result":{"contents":[{"uri":"file:///one","mimeType":"image/png","blob":"AQID"}]}}' ;;
  *'"id":8'*)
    echo '{"jsonrpc":"2.0","id":8,"result":{"description":"review","messages":[{"role":"user","content":{"type":"text","text":"Analyze deeply"}}]}}' ;;
  *)
    echo '{"jsonrpc":"2.0","id":999,"error":{"code":-32601,"message":"unexpected request"}}' ;;
esac
done"#;
    fs::write(
        &config,
        json!({"default_provider":"offline","providers":[{
        "id":"offline","kind":"openai-compatible","model":"unused",
        "base_url":"http://127.0.0.1:9/v1"
    }],"mcp_servers":[{"id":"catalog","command":"sh","args":["-c",script]}]})
        .to_string(),
    )
    .unwrap();
    let mut process = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = process.stdin.take().unwrap();
        for value in [
            json!({"type":"control_request","request_id":"resources",
                "request":{"subtype":"mcp_resources"}}),
            json!({"type":"control_request","request_id":"prompts",
                "request":{"subtype":"mcp_prompts"}}),
            json!({"type":"control","request_id":"templates",
                "action":"mcp_resource_templates"}),
            json!({"type":"control","request_id":"read","action":"mcp_read_resource",
                "server":"catalog","uri":"file:///one"}),
            json!({"type":"control_request","request_id":"prompt",
                "request":{"subtype":"mcp_get_prompt","server":"catalog","name":"inspect",
                    "arguments":{"goal":"reliability"}}}),
            json!({"type":"control_request","request_id":"invalid",
                "request":{"subtype":"mcp_get_prompt","server":"catalog","name":"inspect",
                    "arguments":{"goal":3}}}),
        ] {
            writeln!(input, "{value}").unwrap()
        }
    }
    let out = process.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let outputs = String::from_utf8(out.stdout).unwrap();
    let replies = outputs
        .lines()
        .filter_map(|s| serde_json::from_str::<Value>(s).ok())
        .filter(|v| v["type"] == "control_response")
        .collect::<Vec<_>>();
    let find = |id: &str| {
        replies
            .iter()
            .find(|v| v["request_id"] == id || v["response"]["request_id"] == id)
            .unwrap()
    };
    let resources = &find("resources")["response"]["response"]["servers"][0]["items"];
    assert_eq!(resources.as_array().unwrap().len(), 2, "{outputs}");
    assert_eq!(resources[0]["server"], "catalog");
    assert_eq!(resources[1]["uri"], "file:///two");
    let prompts = &find("prompts")["response"];
    assert!(prompts.to_string().contains("inspect"), "{outputs}");
    assert!(
        find("templates").to_string().contains("file:///{path}"),
        "{outputs}"
    );
    assert!(find("read").to_string().contains("AQID"), "{outputs}");
    assert!(
        find("prompt").to_string().contains("Analyze deeply"),
        "{outputs}"
    );
    assert!(!find("invalid").to_string().contains("Analyze deeply"));
}

#[cfg(unix)]
#[test]
fn delegated_subagent_real_model_run_triggers_start_and_stop_hooks() {
    let root = tempfile::tempdir().unwrap();
    let start_file = root.path().join("start-hook.json");
    let stop_file = root.path().join("stop-hook.json");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut seen = Vec::new();
        for index in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            seen.push(read_request(&mut stream));
            let reply = match index {
                0 => json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"child-1","type":"function","function":{
                        "name":"spawn_agent","arguments":json!({
                            "prompt":"Write a brief findings summary",
                            "subagent_type":"explore"
                        }).to_string()
                    }}]},"finish_reason":"tool_calls"}]}),
                1 => json!({"choices":[{"message":{"role":"assistant",
                    "content":"Child finished investigation"},"finish_reason":"stop"}]}),
                _ => json!({"choices":[{"message":{"role":"assistant",
                    "content":"Parent completed delegation"},"finish_reason":"stop"}]}),
            };
            let body = reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        seen
    });
    let config = root.path().join("model.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible","model":"unit",
                "base_url":format!("http://{address}/v1")}],
            "hooks":[
                {"event":"SubagentStart","matcher":"^explore$",
                    "command":format!("cat > '{}'",start_file.display())},
                {"event":"SubagentStop","matcher":"^explore$",
                    "command":format!("cat > '{}'",stop_file.display())}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let run = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Delegate this task",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("Parent completed delegation"));
    assert!(requests[0].contains("Delegate this task"));
    assert!(requests[1].contains("Write a brief findings summary"));
    assert!(requests[2].contains("Child finished investigation"));
    let start: Value = serde_json::from_slice(&fs::read(&start_file).unwrap()).unwrap();
    let stop: Value = serde_json::from_slice(&fs::read(&stop_file).unwrap()).unwrap();
    assert_eq!(start["hook_event_name"], "SubagentStart");
    assert_eq!(stop["hook_event_name"], "SubagentStop");
    assert_eq!(start["agent_type"], "explore");
    assert_eq!(stop["agent_type"], "explore");
    assert_eq!(start["agent_id"], stop["agent_id"]);
    assert_eq!(
        stop["last_assistant_message"],
        "Child finished investigation"
    );
    assert!(stop["agent_transcript_path"]
        .as_str()
        .unwrap()
        .contains(".atlas/sessions/"));
}

#[cfg(unix)]
#[test]
fn blocking_subagent_start_hook_prevents_child_provider_request() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let reply = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"blocked-child","type":"function","function":{
                        "name":"spawn_agent","arguments":"{\"prompt\":\"must not run\"}"
                    }}]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Delegation rejected"},"finish_reason":"stop"}]})
            };
            let body = reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible","model":"unit",
                "base_url":format!("http://{address}/v1")}],
            "hooks":[{"event":"SubagentStart","command":"echo 'review denied' >&2; exit 2"}]
        })
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Delegation test",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 2);
    // The parent's follow-up request legitimately includes the original tool
    // arguments in its conversation history. Two total HTTP requests (parent
    // original + parent response) and the tool's blocking feedback prove that
    // the child provider was never contacted.
    assert!(requests[1].contains("SubagentStart hook blocked"));
    assert!(requests[1].contains("review denied"));
}

#[cfg(unix)]
#[test]
fn sdk_mcp_resource_subscriptions_preserve_notifications_and_idempotence() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("mcp-subscriptions.json");
    let script = r#"while IFS= read -r line; do
case "$line" in
  *'"id":1'*)
    echo '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"resources":{"subscribe":true}}}}' ;;
  *'"id":2'*)
    echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}' ;;
  *'"id":3'*)
    echo '{"jsonrpc":"2.0","method":"notifications/resources/updated","params":{"uri":"file:///document","revision":2}}'
    echo '{"jsonrpc":"2.0","id":3,"result":{}}' ;;
  *'"id":4'*)
    echo '{"jsonrpc":"2.0","id":4,"result":{}}' ;;
  *)
    echo '{"jsonrpc":"2.0","id":999,"error":{"code":-32601,"message":"unexpected request"}}' ;;
esac
done"#;
    fs::write(
        &config,
        json!({
            "default_provider":"local",
            "providers":[{"id":"local","kind":"openai-compatible","model":"unused",
                "base_url":"http://127.0.0.1:9/v1"}],
            "mcp_servers":[{"id":"documents","command":"sh","args":["-c",script]}]
        })
        .to_string(),
    )
    .unwrap();
    let mut process = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = process.stdin.take().unwrap();
        for value in [
            json!({"type":"control_request","request_id":"subscribe","request":{
                "subtype":"mcp_resource_subscribe","server":"documents",
                "uri":"file:///document"}}),
            json!({"type":"control","request_id":"duplicate",
                "action":"mcp_resource_subscribe","server":"documents",
                "uri":"file:///document"}),
            json!({"type":"control","request_id":"events",
                "action":"mcp_resource_updates"}),
            json!({"type":"control","request_id":"events-again",
                "action":"mcp_resource_updates"}),
            json!({"type":"control_request","request_id":"unsubscribe","request":{
                "subtype":"mcp_resource_unsubscribe","server":"documents",
                "uri":"file:///document"}}),
            json!({"type":"control","request_id":"duplicate-unsub",
                "action":"mcp_resource_unsubscribe","server":"documents",
                "uri":"file:///document"}),
        ] {
            writeln!(input, "{value}").unwrap();
        }
    }
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = String::from_utf8(output.stdout).unwrap();
    let replies = lines
        .lines()
        .map(|x| serde_json::from_str::<Value>(x).unwrap())
        .filter(|x| x["type"] == "control_response")
        .collect::<Vec<_>>();
    let item = |id: &str| {
        replies
            .iter()
            .find(|x| x["request_id"] == id || x["response"]["request_id"] == id)
            .unwrap()
    };
    assert_eq!(
        item("subscribe")["response"]["response"]["subscribed"],
        true,
        "{lines}"
    );
    assert_eq!(item("duplicate")["result"]["already_subscribed"], true);
    assert_eq!(
        item("events")["result"]["updates"][0]["server"],
        "documents"
    );
    assert_eq!(
        item("events")["result"]["updates"][0]["params"]["revision"],
        2
    );
    assert_eq!(
        item("events-again")["result"]["updates"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        item("unsubscribe")["response"]["response"]["subscribed"],
        false
    );
    assert_eq!(
        item("duplicate-unsub")["result"]["already_unsubscribed"],
        true
    );
}

#[cfg(unix)]
#[test]
fn sdk_mcp_prompt_slash_command_binds_arguments_and_sends_real_model_request() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let http = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = http.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = http.accept().unwrap();
        let request = read_request(&mut stream);
        let output = json!({"choices":[{"message":{"role":"assistant",
            "content":"MCP command executed and sent to model."},
            "finish_reason":"stop"}]})
        .to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{output}",output.len()).unwrap();
        stream.flush().unwrap();
        request
    });
    let script = r#"while IFS= read -r line; do
case "$line" in
 *'"id":1'*) echo '{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"prompts"},"capabilities":{"tools":{},"prompts":{}}}}' ;;
 *'"id":2'*) echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}' ;;
 *'"id":3'*) echo '{"jsonrpc":"2.0","id":3,"result":{"prompts":[{"name":"review","description":"Review a change","arguments":[{"name":"goal"},{"name":"mode"}]}]}}' ;;
 *'"id":5'*) echo '{"jsonrpc":"2.0","id":5,"result":{"prompts":[{"name":"review","description":"Review a change","arguments":[{"name":"goal"},{"name":"mode"}]}]}}' ;;
 *'"id":4'*)
   case "$line" in
     *'"goal":"alpha"'*'"mode":"beta"'*)
       echo '{"jsonrpc":"2.0","id":4,"result":{"messages":[{"role":"user","content":{"type":"text","text":"review alpha beta"}},{"role":"assistant","content":{"type":"text","text":"Use safe checks"}}]}}' ;;
     *)
       echo '{"jsonrpc":"2.0","id":4,"error":{"code":-32602,"message":"wrong prompt arguments"}}' ;;
   esac ;;
esac
done"#;
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","base_url":format!("http://{addr}/v1"),
        "model":"mcp-test"
    }],"mcp_servers":[{"id":"docs","command":"sh","args":["-c",script]}]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({"type":"user","content":
            "/docs:review (MCP) alpha beta"})
        )
        .unwrap();
        writeln!(
            input,
            "{}",
            json!({"type":"control",
            "action":"mcp_prompt_commands","request_id":"listing"})
        )
        .unwrap();
    }
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(
        output.contains("MCP command executed and sent to model."),
        "{output}"
    );
    assert!(output.contains("mcp__docs__review"), "{output}");
    assert!(output.contains("docs:review (MCP)"), "{output}");
    let request = server.join().unwrap();
    assert!(request.contains("review alpha beta"), "{request}");
    assert!(request.contains("Use safe checks"), "{request}");
    assert!(!request.contains("/docs:review (MCP)"), "{request}");
}

#[test]
fn chat_additional_directory_requires_confirmation_and_persists_only_when_requested() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let folder = external.path().join("folder with spaces");
    fs::create_dir(&folder).unwrap();
    let approved = folder.canonicalize().unwrap();
    let config = root.path().join("outside-config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"local",
            "providers":[{"id":"local","kind":"openai-compatible",
                "base_url":"http://127.0.0.1:9/v1","model":"offline"}]
        })
        .to_string(),
    )
    .unwrap();
    let run = |instructions: String| {
        let mut child = binary()
            .args(["--workspace", root.path().to_str().unwrap(), "chat"])
            .env("ATLAS_CONFIG", &config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(instructions.as_bytes()).unwrap();
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let rejected = run(format!(
        "/add-dir {}\nn\n/dirs\n/exit\n",
        approved.display()
    ));
    assert!(rejected.contains("Did not add"), "{rejected}");
    assert!(
        !serde_json::from_str::<Value>(&fs::read_to_string(&config).unwrap())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("additional_directories")
    );
    let accepted = run(format!(
        "/add-dir {} --remember\ny\n/dirs\n/exit\n",
        approved.display()
    ));
    assert!(accepted.contains("saved to local settings"), "{accepted}");
    let saved: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    assert_eq!(
        saved["additional_directories"][0],
        approved.to_str().unwrap()
    );
    let resumed = run("/dirs\n/exit\n".to_string());
    assert!(resumed.contains(approved.to_str().unwrap()), "{resumed}");
}

#[test]
fn sdk_multimodal_direct_and_queued_messages_reach_model_and_survive_session_restore() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for turn in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            let request = read_request(&mut socket);
            requests.push(request);
            let response = json!({"choices":[{"message":{"role":"assistant",
                "content":if turn==0{"direct image ok"}else{"queued image ok"}},
                "finish_reason":"stop"}]})
            .to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),response).unwrap();
            socket.flush().unwrap();
        }
        requests
    });
    let config = dir.path().join("models.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"vision-test",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let image = || {
        json!({"type":"image","source":{
            "type":"base64","media_type":"image/png","data":"AQID"
        }})
    };
    let direct = json!({"type":"user","content":[
        {"type":"text","text":"Describe direct picture"},image()
    ]});
    let queued = json!({"type":"control","action":"queue_enqueue",
        "command":{"value":[{"type":"text","text":"Describe queued picture"},image()],
            "mode":"prompt","priority":"next"}});
    let drain = json!({"type":"control","action":"queue_drain","max_priority":"next"});
    let mut child = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for event in [direct, queued, drain] {
            writeln!(stdin, "{event}").unwrap()
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8(output.stdout).unwrap();
    assert!(wire.contains("direct image ok"), "{wire}");
    assert!(wire.contains("queued image ok"), "{wire}");
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let lower = request.to_ascii_lowercase();
        assert!(lower.contains("data:image/png;base64,aqid"), "{request}");
        assert!(request.contains("image_url"), "{request}");
        assert!(!request.contains("content_blocks"), "{request}");
    }
    assert!(requests[0].contains("Describe direct picture"));
    assert!(requests[1].contains("Describe queued picture"));
    let session_dir = dir.path().join(".atlas/sessions");
    let paths = fs::read_dir(session_dir).unwrap().collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    let saved = fs::read_to_string(paths[0].as_ref().unwrap().path()).unwrap();
    assert!(saved.contains("content_blocks"));
    assert!(saved.contains("AQID"));
    assert!(saved.contains("Describe queued picture"));
}

#[test]
fn command_line_extra_directory_reaches_real_model_file_tool() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let target = external.path().join("outside.txt");
    fs::write(&target, "External workspace content verified").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let message = if step == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"extra-read","type":"function","function":{
                        "name":"read_file","arguments":json!({"path":target}).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"External directory read completed",
                    "tool_calls":[]},"finish_reason":"stop"}]})
            };
            let body = message.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test-model",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let result = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--add-dir",
            external.path().to_str().unwrap(),
            "run",
            "Read the externally authorized file",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("External directory read completed"));
    assert!(
        requests[1].contains("External workspace content verified"),
        "Second model request did not receive the actual external file: {}",
        requests[1]
    );
}

#[test]
fn chat_add_dir_without_argument_prompts_for_path_and_remember_choice() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible",
            "base_url":"http://127.0.0.1:9/v1","model":"offline"
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "/add-dir").unwrap();
        writeln!(input, "{}", external.path().display()).unwrap();
        writeln!(input, "r").unwrap();
        writeln!(input, "/exit").unwrap();
    }
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let shown = String::from_utf8(result.stdout).unwrap();
    assert!(shown.contains("Directory path:"), "{shown}");
    assert!(shown.contains("saved to local settings"), "{shown}");
    let saved: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    assert_eq!(
        saved["additional_directories"][0],
        external.path().canonicalize().unwrap().to_str().unwrap()
    );
}

#[test]
fn sdk_batches_same_workload_text_and_image_as_one_model_turn_without_losing_uuids() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let received = thread::spawn(move || {
        let (mut conn, _) = server.accept().unwrap();
        let request = read_request(&mut conn);
        let response = json!({"choices":[{"message":{"role":"assistant",
            "content":"batched vision ok"},"finish_reason":"stop"}]})
        .to_string();
        write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),response).unwrap();
        conn.flush().unwrap();
        request
    });
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"vision",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = sdk.stdin.take().unwrap();
        let image = json!({"type":"image","source":{
            "type":"base64","media_type":"image/png","data":"AQID"
        }});
        for event in [
            json!({"type":"user","queued":true,"uuid":"img-1","priority":"next",
                "workload":"review","content":[{"type":"text","text":"first visual"},image]}),
            json!({"type":"user","queued":true,"uuid":"text-2","priority":"next",
                "workload":"review","content":"second instruction"}),
            json!({"type":"control","action":"queue_drain","request_id":"drain",
                "max_priority":"next","replay_user_messages":true}),
        ] {
            writeln!(stdin, "{event}").unwrap();
        }
    }
    let output = sdk.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8(output.stdout).unwrap();
    let events = wire
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let replay = events
        .iter()
        .filter(|v| v["type"] == "user" && v["isReplay"] == true)
        .map(|v| v["uuid"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(replay, vec!["img-1", "text-2"]);
    let drain = events.iter().find(|v| v["request_id"] == "drain").unwrap();
    assert_eq!(drain["ok"], true, "{wire}");
    assert_eq!(drain["result"]["results"].as_array().unwrap().len(), 2);
    assert_eq!(drain["result"]["length"], 0);
    let http = received.join().unwrap();
    assert!(http.contains("first visual"));
    assert!(http.contains("second instruction"));
    assert!(http.contains("data:image/png;base64,AQID"));
}

#[test]
fn authorized_extra_directory_is_inherited_by_subagent_and_team_member() {
    let extra = tempfile::tempdir().unwrap();
    let textfile = extra.path().join("shared.txt");
    fs::write(&textfile, "Shared directory content for child agents").unwrap();
    for tool_name in ["spawn_agent", "spawn_team"] {
        let root = tempfile::tempdir().unwrap();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let file = textfile.clone();
        let tool_name = tool_name.to_string();
        let server_task = thread::spawn(move || {
            let mut requests = Vec::new();
            for step in 0..4 {
                let (mut connection, _) = server.accept().unwrap();
                requests.push(read_request(&mut connection));
                let message = match step {
                    0 => {
                        let arguments = if tool_name == "spawn_agent" {
                            json!({"prompt":"Read authorized external directory"})
                        } else {
                            json!({"tasks":[{"name":"worker","prompt":"Read authorized external directory"}]})
                        };
                        json!({"role":"assistant","content":null,"tool_calls":[{
                            "id":"delegate-1","type":"function","function":{
                                "name":tool_name,"arguments":arguments.to_string()
                            }
                        }]})
                    }
                    1 => json!({"role":"assistant","content":null,"tool_calls":[{
                        "id":"child-read","type":"function","function":{
                            "name":"read_file","arguments":json!({"path":file}).to_string()
                        }
                    }]}),
                    2 => json!({"role":"assistant","content":"Child completed external read"}),
                    _ => json!({"role":"assistant","content":"Parent completed delegation"}),
                };
                let finish = if step < 2 { "tool_calls" } else { "stop" };
                let body =
                    json!({"choices":[{"message":message,"finish_reason":finish}]}).to_string();
                write!(connection,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),body).unwrap();
                connection.flush().unwrap();
            }
            requests
        });
        let config = root.path().join("config.json");
        fs::write(
            &config,
            json!({"default_provider":"mock","providers":[{
                "id":"mock","kind":"openai-compatible","model":"test",
                "base_url":format!("http://{address}/v1")
            }]})
            .to_string(),
        )
        .unwrap();
        let output = binary()
            .args([
                "--workspace",
                root.path().to_str().unwrap(),
                "--add-dir",
                extra.path().to_str().unwrap(),
                "--approval",
                "read-only",
                "run",
                "Delegate the external file read",
            ])
            .env("ATLAS_CONFIG", &config)
            .output()
            .unwrap();
        let requests = server_task.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            requests[2].contains("Shared directory content for child agents"),
            "Child model did not receive authorized file content: {}",
            requests[2]
        );
        assert!(
            requests[3].contains("Child completed external read"),
            "{}",
            requests[3]
        );
    }
}

#[cfg(unix)]
#[test]
fn sdk_mcp_prompt_image_content_flows_to_real_model_multimodal_request() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let http = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let request = read_request(&mut socket);
        let response = json!({"choices":[{"message":{"role":"assistant",
            "content":"MCP vision complete"},"finish_reason":"stop"}]})
        .to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),response).unwrap();
        socket.flush().unwrap();
        request
    });
    let script = r#"while IFS= read -r line; do
case "$line" in
 *'"id":1'*) echo '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"prompts":{},"tools":{}}}}' ;;
 *'"id":2'*) echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}' ;;
 *'"id":3'*) echo '{"jsonrpc":"2.0","id":3,"result":{"prompts":[{"name":"diagram","arguments":[{"name":"topic"}]}]}}' ;;
 *'"id":4'*) echo '{"jsonrpc":"2.0","id":4,"result":{"messages":[{"role":"user","content":[{"type":"text","text":"Analyze the attached image"},{"type":"image","mimeType":"image/png","data":"AQID"}]}]}}' ;;
esac
done"#;
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"vision",
        "base_url":format!("http://{address}/v1")
    }],"mcp_servers":[{"id":"docs","command":"sh","args":["-c",script]}]})
        .to_string(),
    )
    .unwrap();
    let mut sdk = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = sdk.stdin.take().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"type":"user",
            "content":"/docs:diagram (MCP) inspection"})
        )
        .unwrap();
    }
    let output = sdk.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8(output.stdout).unwrap();
    assert!(wire.contains("MCP vision complete"), "{wire}");
    let request = http.join().unwrap();
    assert!(request.contains("Analyze the attached image"));
    assert!(request.contains("data:image/png;base64,AQID"));
    assert!(!request.contains("/docs:diagram (MCP)"));
}

#[test]
fn cli_session_list_rename_export_and_workspace_safety() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let config = root.join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[]}).to_string(),
    )
    .unwrap();
    let id = uuid::Uuid::new_v4();
    let dir = root.join(".atlas/sessions");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{id}.json")),
        json!({
            "id":id,"messages":[
                {"role":"user","content":"Build a complete Rust assistant"},
                {"role":"assistant","content":"Proceeding with implementation"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let args = ["--workspace", root.to_str().unwrap()];
    let list = binary()
        .args(args)
        .arg("sessions")
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    assert!(String::from_utf8_lossy(&list.stdout).contains("Build a complete Rust assistant"));
    let rename = binary()
        .args(args)
        .args(["rename", &id.to_string(), "Rust parity project"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        rename.status.success(),
        "{}",
        String::from_utf8_lossy(&rename.stderr)
    );
    let after: Value =
        serde_json::from_slice(&fs::read(dir.join(format!("{id}.json"))).unwrap()).unwrap();
    assert_eq!(after["custom_title"], "Rust parity project");
    let list = binary()
        .args(args)
        .arg("sessions")
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&list.stdout).contains("Rust parity project"));
    let export = binary()
        .args(args)
        .args(["export", &id.to_string(), "history.md"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let body = fs::read_to_string(root.join("history.txt")).unwrap();
    assert!(body.contains("Build a complete Rust assistant"));
    assert!(body.contains("Proceeding with implementation"));
    let external = tempfile::tempdir().unwrap();
    let bad = binary()
        .args(args)
        .args([
            "export",
            &id.to_string(),
            external.path().join("out.txt").to_str().unwrap(),
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(!external.path().join("out.txt").exists());
}

#[test]
fn resume_by_unambiguous_title_uses_real_persisted_session_without_network() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    let init = binary()
        .arg("init")
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let id = uuid::Uuid::new_v4();
    let sessions = root.path().join(".atlas/sessions");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join(format!("{id}.json")),
        json!({
            "id":id,"custom_title":"Unique Continuation Topic",
            "messages":[{"role":"user","content":"Continue verified Rust migration"}]
        })
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "chat",
            "--resume",
            "continuation topic",
        ])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "/session\n/exit").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(&id.to_string()));
}

#[test]
fn chat_auto_names_session_using_selected_provider_without_adding_chat_turn() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let id = uuid::Uuid::new_v4();
    let directory = root.path().join(".atlas/sessions");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join(format!("{id}.json")),
        json!({
            "id":id,"messages":[
                {"role":"user","content":"Fix flaky integration tests"},
                {"role":"assistant","content":"I checked the Rust test path"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let service = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        assert!(request.contains("Fix flaky integration tests"));
        assert!(request.contains("kebab-case name"));
        let body = json!({"choices":[{"message":{
            "role":"assistant","content":r#"{"name":"fix-flaky-tests"}"#
        },"finish_reason":"stop"}],
        "usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}})
        .to_string();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        stream.flush().unwrap();
    });
    let cfg = root.path().join("config.json");
    fs::write(
        &cfg,
        json!({"default_provider":"mock",
        "providers":[{"id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"test-model"}]})
        .to_string(),
    )
    .unwrap();
    let mut cli = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "chat",
            "--resume",
            &id.to_string(),
        ])
        .env("ATLAS_CONFIG", &cfg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = cli.stdin.take().unwrap();
        writeln!(input, "/rename\n/exit").unwrap();
    }
    let result = cli.wait_with_output().unwrap();
    service.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let session: Value =
        serde_json::from_slice(&fs::read(directory.join(format!("{id}.json"))).unwrap()).unwrap();
    assert_eq!(session["custom_title"], "fix-flaky-tests");
    assert_eq!(session["messages"].as_array().unwrap().len(), 2);
    assert!(session["usage"]["total_tokens"].as_u64().unwrap_or(0) >= 7);
}

#[cfg(unix)]
#[test]
fn configured_powershell_uses_real_shell_protocol_and_delivers_tool_output_to_model() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let fake_bin = root.path().join("bin");
    fs::create_dir(&fake_bin).unwrap();
    let fake_shell = fake_bin.join("pwsh");
    fs::write(
        &fake_shell,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" \"$2\" \"$3\" > \"$ATLAS_PS_FLAGS\"\nexec sh -c \"$4\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake_shell, fs::Permissions::from_mode(0o755)).unwrap();
    let flags = root.path().join("flags.txt");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for i in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let msg = if i == 0 {
                json!({"role":"assistant","content":null,"tool_calls":[{
                "id":"shell-call","type":"function","function":{
                    "name":"run_command","arguments":json!({"command":"printf 'shell-command-success'"}).to_string()
                }}]})
            } else {
                json!({"role":"assistant","content":"PowerShell execution verified"})
            };
            let body=json!({"choices":[{"message":msg,"finish_reason":if i==0 {"tool_calls"} else {"stop"}}]}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","default_shell":"powershell",
        "providers":[{"id":"mock","kind":"openai-compatible","model":"model",
        "base_url":format!("http://{addr}/v1")}]})
        .to_string(),
    )
    .unwrap();
    let inherited = std::env::var_os("PATH").unwrap();
    let composed = std::env::join_paths(
        std::iter::once(fake_bin.as_path()).chain(
            std::env::split_paths(&inherited)
                .collect::<Vec<_>>()
                .iter()
                .map(|p| p.as_path()),
        ),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "test configured shell",
        ])
        .env("ATLAS_CONFIG", &config)
        .env("PATH", composed)
        .env("ATLAS_PS_FLAGS", &flags)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("PowerShell execution verified"));
    assert!(
        requests[1].contains("shell-command-success"),
        "{}",
        requests[1]
    );
    assert_eq!(
        fs::read_to_string(flags).unwrap(),
        "-NoProfile\n-NonInteractive\n-Command\n"
    );
}
#[test]
fn invalid_shell_selection_fails_before_model_request() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","default_shell":"cmd",
        "providers":[{"id":"mock","kind":"openai-compatible","model":"unused",
        "base_url":"http://127.0.0.1:1/v1"}]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "run", "hello"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unsupported default shell"));
}

#[test]
fn chat_session_environment_is_used_by_real_shell_tool_and_can_be_unset() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..4 {
            let (mut client, _) = listener.accept().unwrap();
            requests.push(read_request(&mut client));
            let message = if step == 0 || step == 2 {
                json!({"role":"assistant","content":null,"tool_calls":[{
                    "id":format!("shell-{step}"),"type":"function","function":{
                        "name":"run_command","arguments":json!({"command":"printenv ATLAS_SESSION_TEST"}).to_string()
                    }
                }]})
            } else {
                json!({"role":"assistant","content":"Shell environment request processed"})
            };
            let body = json!({"choices":[{"message":message,
                "finish_reason":if step%2==0 {"tool_calls"} else {"stop"}}]})
            .to_string();
            write!(client,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            client.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"mock",
            "base_url":format!("http://{addr}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "chat",
        ])
        .env("ATLAS_CONFIG", &config)
        .env_remove("ATLAS_SESSION_TEST")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        write!(input,"/env ATLAS_SESSION_TEST=verified-value\nCheck environment\n/env -u ATLAS_SESSION_TEST\nCheck again\n/exit\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(requests[1].contains("verified-value"), "{}", requests[1]);
    let last: Value = serde_json::from_str(requests[3].split_once("\r\n\r\n").unwrap().1).unwrap();
    let tool_result = last["messages"].as_array().unwrap().last().unwrap();
    assert!(
        tool_result["content"]
            .as_str()
            .unwrap_or("")
            .contains("STDOUT:"),
        "{tool_result}"
    );
    assert!(
        !tool_result["content"]
            .as_str()
            .unwrap_or("")
            .contains("verified-value"),
        "{tool_result}"
    );
    let saved = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    assert!(saved.iter().all(|s| !s.contains("session_env")));
}

#[test]
fn subprocess_secret_scrub_blocks_model_provider_credential_from_shell_tool() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..2 {
            let (mut connection, _) = listener.accept().unwrap();
            requests.push(read_request(&mut connection));
            let reply = if step == 0 {
                json!({"role":"assistant","content":null,"tool_calls":[{
                    "id":"inspect-env","type":"function","function":{
                        "name":"run_command","arguments":json!({
                            "command":"printenv OPENAI_API_KEY || printf 'credential-not-present'"
                        }).to_string()
                    }
                }]})
            } else {
                json!({"role":"assistant","content":"Secret isolation checked"})
            };
            let body = json!({"choices":[{"message":reply,
                "finish_reason":if step==0 {"tool_calls"} else {"stop"}}]})
            .to_string();
            write!(connection,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            connection.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "full",
            "run",
            "Inspect credential visibility",
        ])
        .env("ATLAS_CONFIG", &config)
        .env("ATLAS_SUBPROCESS_ENV_SCRUB", "true")
        .env("OPENAI_API_KEY", "SENSITIVE_TEST_VALUE")
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        requests[1].contains("credential-not-present"),
        "{}",
        requests[1]
    );
    assert!(!requests[1].contains("SENSITIVE_TEST_VALUE"));
}

#[test]
fn sdk_polls_independent_http_mcp_get_resource_notifications_end_to_end() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..4 {
            let (mut socket, _) = listener.accept().unwrap();
            let input = read_request(&mut socket);
            requests.push(input);
            let (kind, body) = match step {
                0 => (
                    "application/json",
                    json!({"jsonrpc":"2.0","id":1,
                    "result":{"serverInfo":{"name":"push","version":"1"},
                        "capabilities":{"tools":{},"resources":{"subscribe":true}}}})
                    .to_string(),
                ),
                1 => ("application/json", String::new()),
                2 => (
                    "application/json",
                    json!({"jsonrpc":"2.0","id":2,
                    "result":{"tools":[]}})
                    .to_string(),
                ),
                _ => (
                    "text/event-stream",
                    format!(
                        "id: new-state\r\nevent: message\r\ndata: {}\r\n\r\n",
                        json!({"jsonrpc":"2.0",
                        "method":"notifications/resources/updated",
                        "params":{"uri":"file:///sdk-push","revision":7}})
                    ),
                ),
            };
            let status = if step == 1 { "202 Accepted" } else { "200 OK" };
            write!(socket,"HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{body}",
                body.len(),if step==0{"Mcp-Session-Id: sdk-session\r\n"}else{""})
                .unwrap();
            socket.flush().unwrap();
        }
        requests
    });
    let config = dir.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"offline",
            "providers":[{"id":"offline","kind":"openai-compatible",
                "model":"offline","base_url":"http://127.0.0.1:9/v1"}],
            "mcp_servers":[{"id":"push","type":"http",
                "url":format!("http://{address}/mcp")}]
        })
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({"type":"control_request","request_id":"poll",
            "request":{"subtype":"mcp_poll_resource_updates","max_wait_ms":3000}})
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = String::from_utf8(output.stdout).unwrap();
    let response = events
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|event| event["response"]["request_id"] == "poll")
        .unwrap();
    assert_eq!(response["response"]["subtype"], "success", "{response}");
    assert_eq!(
        response["response"]["response"]["updates"][0]["server"],
        "push"
    );
    assert_eq!(
        response["response"]["response"]["updates"][0]["params"]["revision"],
        7
    );
    assert_eq!(response["response"]["response"]["errors"], json!([]));
    let requests = server.join().unwrap();
    assert!(requests[3].starts_with("GET /mcp HTTP/1.1"));
    assert!(requests[3]
        .to_ascii_lowercase()
        .contains("mcp-session-id: sdk-session"));
}

#[test]
fn interactive_permission_settings_persist_and_reload_without_model_calls() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible","model":"test",
            "base_url":"http://127.0.0.1:9/v1","stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    let local = root.path().join(".atlas/settings.local.json");
    fs::create_dir_all(local.parent().unwrap()).unwrap();
    fs::write(&local, json!({"hooks":{"keep":"untouched"}}).to_string()).unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .env("ATLAS_USER_SETTINGS", root.path().join("user.json"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(
        b"/permission add local allow Bash(touch allowed)\n/permission add local allow Bash(touch allowed)\n/permission delete local allow Bash(touch allowed)\n/exit\n"
    ).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Permission updated"), "{stdout}");
    let settings: Value = serde_json::from_slice(&fs::read(&local).unwrap()).unwrap();
    assert_eq!(settings["hooks"]["keep"], "untouched");
    assert_eq!(settings["permissions"]["allow"], json!([]));
}

#[test]
fn interactive_managed_only_policy_blocks_persistent_grants() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible","model":"test",
            "base_url":"http://127.0.0.1:9/v1","stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    let policy = root.path().join("managed.json");
    fs::write(
        &policy,
        json!({"allowManagedPermissionRulesOnly":true,
            "permissions":{"deny":["Bash(touch forbidden)"]}})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .env("ATLAS_USER_SETTINGS", root.path().join("user.json"))
        .env("ATLAS_POLICY_SETTINGS", &policy)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/permission add local allow Bash(touch forbidden)\n/exit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("policy-locked"));
    assert!(!root.path().join(".atlas/settings.local.json").exists());
}

#[test]
fn tool_search_model_roundtrip_uses_registered_catalog() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            requests.push(raw);
            let payload = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"find-1","type":"function","function":{
                        "name":"ToolSearch",
                        "arguments":json!({"query":"select:read_file,missing,read_file"}).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant","content":"Search completed."},
                    "finish_reason":"stop"}]})
            };
            let body = payload.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("model.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{addr}/v1"),"stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "--approval",
            "read-only",
            "run",
            "Find a tool",
        ])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Search completed."));
    assert!(
        requests[0].contains("ToolSearch"),
        "ToolSearch is not registered"
    );
    let second: Value =
        serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    let returned = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|msg| msg["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(
        returned.contains("read_file"),
        "actual search result not returned"
    );
    assert!(
        returned.contains("total_deferred_tools"),
        "search result schema missing"
    );
    assert!(!requests[1].contains("Unknown tool: ToolSearch"));
    let session_file = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = fs::read_to_string(session_file).unwrap();
    assert!(
        saved.contains("find-1") && saved.contains("read_file"),
        "tool response not persisted"
    );
}

#[test]
fn ask_user_question_chat_uses_actual_terminal_selection() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            requests.push(raw);
            let payload = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"question-1","type":"function","function":{
                        "name":"AskUserQuestion",
                        "arguments":json!({"questions":[{
                            "question":"Which database?","header":"Database",
                            "options":[{"label":"SQLite","description":"Local"},
                                       {"label":"Postgres","description":"Remote"}]
                        }],"answers":{"Which database?":"Spoofed"}}).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant","content":"Answer recorded."},
                    "finish_reason":"stop"}]})
            };
            let body = payload.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("model.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test",
            "base_url":format!("http://{addr}/v1"),"stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"Help me choose\n2\n/exit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(requests[0].contains("AskUserQuestion"));
    let second: Value =
        serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    let returned = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|msg| msg["role"] == "tool")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(
        returned.contains("Postgres"),
        "terminal answer not returned to model"
    );
    assert!(
        !returned.contains("Spoofed"),
        "model-provided answer accepted"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Which database?"));
}

#[test]
fn chat_status_reports_live_state_and_never_prints_secrets() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"portable-model",
            "base_url":"https://example.test/v1?token=URL_SECRET_NEVER_PRINT",
            "api_key_env":"STATUS_TEST_SECRET"
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .env("STATUS_TEST_SECRET", "API_SECRET_NEVER_PRINT")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/status\n/rename fixed-status-test\n/status\n/exit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("Session name: (unnamed)"), "{stdout}");
    assert!(
        stdout.contains("Session name: fixed-status-test"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Provider: mock (openai-compatible)"),
        "{stdout}"
    );
    assert!(stdout.contains("Model: portable-model"), "{stdout}");
    assert!(stdout.contains("Permission mode: default"), "{stdout}");
    assert!(stdout.contains("MCP servers: 0"), "{stdout}");
    assert!(stdout.contains(root.path().to_str().unwrap()), "{stdout}");
    assert!(!stdout.contains("API_SECRET_NEVER_PRINT"));
    assert!(!stdout.contains("URL_SECRET_NEVER_PRINT"));
    assert!(!stderr.contains("API_SECRET_NEVER_PRINT"));
    assert!(!stderr.contains("URL_SECRET_NEVER_PRINT"));
}

#[test]
fn transcript_search_uses_visible_messages_not_model_only_reminders() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join(".atlas/sessions");
    fs::create_dir_all(&directory).unwrap();
    let id = "3c4d57ae-1a1c-474b-854d-09ad8fd1123e";
    let session = json!({
        "id": id,
        "messages": [
            {"role":"system","content":"hidden system needle"},
            {"role":"user","content":"Café patch <system-reminder>private needle</system-reminder> file"},
            {"role":"assistant","content":"Fixed café controller"},
            {"role":"tool","content":"{\"stdout\":\"visible grep result\",\"rawOutputPath\":\"needle\"}"},
            {"role":"user","content":"[Request interrupted by user]"}
        ]
    });
    fs::write(directory.join(format!("{id}.json")), session.to_string()).unwrap();
    let output = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "transcript-search",
            id,
            "CAFÉ",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("2  user"));
    assert!(text.contains("3  assistant"));
    assert!(!text.contains("hidden system"));
    let phantom = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "transcript-search",
            id,
            "needle",
        ])
        .output()
        .unwrap();
    assert!(phantom.status.success());
    assert!(
        phantom.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&phantom.stdout)
    );
    let tool = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "transcript-search",
            id,
            "visible grep",
        ])
        .output()
        .unwrap();
    assert!(tool.status.success());
    assert!(String::from_utf8_lossy(&tool.stdout).contains("4  tool"));
}

#[test]
fn doctor_cli_and_chat_report_real_local_environment_offline() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("AGENTS.md"), "a".repeat(50_001)).unwrap();
    let config = root.path().join("settings.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"offline",
            "base_url":"http://127.0.0.1:9/v1"
        }],"mcp_servers":[{
            "id":"missing","command":"fake-executable-for-doctor-test"
        }]})
        .to_string(),
    )
    .unwrap();
    let output = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "doctor"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Version:"), "{text}");
    assert!(text.contains("Provider: 1 model profile"), "{text}");
    assert!(text.contains("MCP: missing: command"), "{text}");
    assert!(text.contains("Context: AGENTS.md is 50001 bytes"), "{text}");
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/doctor\n/exit\n")
        .unwrap();
    let chat = child.wait_with_output().unwrap();
    assert!(
        chat.status.success(),
        "{}",
        String::from_utf8_lossy(&chat.stderr)
    );
    let chat_text = String::from_utf8_lossy(&chat.stdout);
    assert!(
        chat_text.contains("Context: AGENTS.md is 50001 bytes"),
        "{chat_text}"
    );
    assert!(
        chat_text.contains("Live MCP: missing: failed"),
        "{chat_text}"
    );
}

#[test]
fn doctor_inspects_invalid_config_without_starting_model() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("bad-config.json");
    fs::write(&config, "{this is invalid JSON}").unwrap();
    let output = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "doctor"])
        .env("ATLAS_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("[WARN] Configuration:"), "{text}");
    assert!(text.contains("settings could not be loaded"), "{text}");
}

#[test]
fn chat_tag_toggles_with_confirmation_and_is_searchable_after_resume() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"local",
            "providers":[{"id":"local","kind":"openai-compatible",
                "model":"unused","base_url":"http://127.0.0.1:9/v1"}]
        })
        .to_string(),
    )
    .unwrap();
    let mut process = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    process.stdin.take().unwrap().write_all(
        b"/tag bugfix\n/sessions #bugfix\n/tag bugfix\nn\n/tag bugfix\ny\n/tag feature\n/status\n/exit\n"
    ).unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Tagged session with #bugfix"), "{stdout}");
    assert!(stdout.contains("Kept tag #bugfix"), "{stdout}");
    assert!(stdout.contains("Removed session tag"), "{stdout}");
    assert!(stdout.contains("Tagged session with #feature"), "{stdout}");
    assert!(stdout.contains("Session tag: #feature"), "{stdout}");
    assert!(stdout.contains("Untitled conversation #bugfix"), "{stdout}");
    let stored = fs::read_dir(root.path().join(".atlas/sessions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let doc: Value = serde_json::from_str(&fs::read_to_string(&stored).unwrap()).unwrap();
    assert_eq!(doc["tag"], "feature");
    let id = doc["id"].as_str().unwrap().to_owned();
    let mut resumed = binary()
        .args([
            "--workspace",
            root.path().to_str().unwrap(),
            "chat",
            "--resume",
            &id,
        ])
        .env("ATLAS_CONFIG", &config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    resumed
        .stdin
        .take()
        .unwrap()
        .write_all(b"/sessions #feature\n/status\n/exit\n")
        .unwrap();
    let result = resumed.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8_lossy(&result.stdout);
    assert!(text.contains("Session tag: #feature"), "{text}");
    assert!(text.contains("Untitled conversation #feature"), "{text}");
}

#[test]
fn chat_branch_preserves_original_conversation_and_uses_collision_free_titles() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let body = json!({"choices":[{"message":{"role":"assistant",
                "content":"Original completion"},"finish_reason":"stop"}]})
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
            requests.push(request);
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"local","providers":[{
            "id":"local","kind":"openai-compatible",
            "model":"test","base_url":format!("http://{address}/v1")
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = binary()
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(
        b"/branch\nResearch branch candidate\n/branch\n/branch\n/branch custom\nOnly in the active branch\n/sessions Branch\n/exit\n"
    ).unwrap();
    let output = child.wait_with_output().unwrap();
    let requests = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 2);
    assert!(requests[0].contains("Research branch candidate"));
    assert!(requests[1].contains("Only in the active branch"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(String::from_utf8_lossy(&output.stderr).contains("No conversation to branch"));
    assert!(
        stdout.contains("Research branch candidate (Branch)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Research branch candidate (Branch 2)"),
        "{stdout}"
    );
    assert!(stdout.contains("custom (Branch)"), "{stdout}");
    let mut sessions = Vec::<Value>::new();
    for entry in fs::read_dir(root.path().join(".atlas/sessions")).unwrap() {
        let entry = entry.unwrap();
        sessions.push(serde_json::from_str(&fs::read_to_string(entry.path()).unwrap()).unwrap());
    }
    assert_eq!(sessions.len(), 4);
    let original = sessions
        .iter()
        .find(|s| s["forked_from"].is_null())
        .unwrap();
    let first = sessions
        .iter()
        .find(|s| s["custom_title"] == "Research branch candidate (Branch)")
        .unwrap();
    let second = sessions
        .iter()
        .find(|s| s["custom_title"] == "Research branch candidate (Branch 2)")
        .unwrap();
    let third = sessions
        .iter()
        .find(|s| s["custom_title"] == "custom (Branch)")
        .unwrap();
    assert_eq!(first["forked_from"], original["id"]);
    assert_eq!(second["forked_from"], first["id"]);
    assert_eq!(third["forked_from"], second["id"]);
    for session in [first, second, third] {
        assert!(session["tag"].is_null());
        assert_ne!(session["id"], original["id"]);
    }
    assert_eq!(first["messages"], original["messages"]);
    assert_eq!(second["messages"], original["messages"]);
    assert_eq!(original["messages"].as_array().unwrap().len(), 2);
    assert_eq!(third["messages"].as_array().unwrap().len(), 4);
    assert_eq!(third["messages"][2]["content"], "Only in the active branch");
}
