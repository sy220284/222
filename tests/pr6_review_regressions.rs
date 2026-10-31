//! Regression coverage for the PR6 source/protocol review: real CLI/SDK,
//! files, permission settings and provider HTTP boundaries.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

enum Reply {
    Json(Value),
    Sse(Vec<Value>),
}
struct Model {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
fn read_request(socket: &mut TcpStream) -> Value {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut block = [0; 8192];
    let end = loop {
        let n = socket.read(&mut block).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&block[..n]);
        if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let length = String::from_utf8_lossy(&bytes[..end])
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    while bytes.len() < end + length {
        let n = socket.read(&mut block).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&block[..n]);
    }
    serde_json::from_slice(&bytes[end..end + length]).unwrap()
}
impl Model {
    fn new(reply: impl Fn(usize, &Value) -> Reply + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, stopped) = (requests.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("accept: {e}"),
                };
                socket.set_nonblocking(false).unwrap();
                let request = read_request(&mut socket);
                let index = seen.lock().unwrap().len();
                let response = reply(index, &request);
                seen.lock().unwrap().push(request);
                let (mime, body) = match response {
                    Reply::Json(value) => ("application/json", value.to_string()),
                    Reply::Sse(events) => (
                        "text/event-stream",
                        events
                            .iter()
                            .map(|event| format!("data: {event}\n\n"))
                            .collect::<String>()
                            + "data: [DONE]\n\n",
                    ),
                };
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        Self {
            url,
            requests,
            stop,
            worker: Some(worker),
        }
    }
    fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn config(root: &Path, model: &Model, kind: &str, stream: bool, rules: Value) {
    fs::write(root.join("config.json"), json!({
        "default_provider":"mock", "providers":[{
            "id":"mock", "kind":kind,"base_url":model.url,"model":"mock",
            "stream":stream, "api_key_env":if kind == "gemini" {json!("PR6_MOCK_KEY")} else {Value::Null}
        }],"permission_rules":rules
    }).to_string()).unwrap();
}
fn run(
    root: &Path,
    args: &[&str],
    input: Option<Value>,
    configure: impl FnOnce(&mut Command),
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atlas-agent"));
    command
        .args(["--workspace", root.to_str().unwrap(), "--approval", "full"])
        .args(args)
        .env("ATLAS_CONFIG", root.join("config.json"))
        .env("PR6_MOCK_KEY", "dummy-test-key")
        .env("ATLAS_STREAMING_TOOL_EXECUTION", "true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut command);
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        writeln!(child.stdin.take().unwrap(), "{input}").unwrap();
    } else {
        drop(child.stdin.take());
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("CLI exceeded deadline");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}
fn done() -> Value {
    json!({"choices":[{"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}]})
}
fn tool(name: &str, arguments: Value, id: &str) -> Value {
    json!({"id":id,"type":"function","function":{"name":name,"arguments":arguments.to_string()}})
}

#[test]
fn sdk_attachments_obey_read_deny_and_ask_before_model_request() {
    for behavior in ["deny", "ask"] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("secret.txt"), "denied-private-body").unwrap();
        let model = Model::new(|_, _| Reply::Json(done()));
        config(
            root.path(),
            &model,
            "openai-compatible",
            false,
            json!([{"behavior":behavior,"rule":"Read(secret.txt)"}]),
        );
        let output = run(
            root.path(),
            &["sdk"],
            Some(json!({
                "type":"user","content":"Inspect file","file_attachments":[{"path":"secret.txt"}]
            })),
            |_| {},
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("error"), "{stdout}");
        assert!(
            model.requests().is_empty(),
            "{behavior} must prevent sending file bytes"
        );
    }
}

#[test]
fn streamed_interleaved_tool_headers_wait_for_complete_arguments() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("one"), "first-file-body").unwrap();
    fs::write(root.path().join("two"), "second-file-body").unwrap();
    let model = Model::new(|round, _| {
        if round == 0 {
            Reply::Sse(vec![
                json!({"choices":[{"delta":{"tool_calls":[
                    {"index":0,"id":"call-a","type":"function","function":{"name":"read_file","arguments":""}},
                    {"index":1,"id":"call-b","type":"function","function":{"name":"read_file","arguments":""}}
                ]}}]}),
                json!({"choices":[{"delta":{"tool_calls":[
                    {"index":0,"function":{"arguments":"{\"path\":\"one\"}"}},
                    {"index":1,"function":{"arguments":"{\"path\":\"two\"}"}}
                ]}}]}),
                json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
            ])
        } else {
            Reply::Sse(vec![
                json!({"choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}),
            ])
        }
    });
    config(root.path(), &model, "openai-compatible", true, json!([]));
    let output = run(
        root.path(),
        &["sdk"],
        Some(json!({"type":"user","content":"Read both"})),
        |_| {},
    );
    assert!(output.status.success());
    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    let results = requests[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert!(
        results[0]["content"]
            .as_str()
            .unwrap()
            .contains("first-file-body"),
        "{results:?}"
    );
    assert!(
        results[1]["content"]
            .as_str()
            .unwrap()
            .contains("second-file-body"),
        "{results:?}"
    );
}

#[test]
fn gemini_native_signatures_survive_tools_session_save_and_reload() {
    for stream in [false, true] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("one"), "first-file-body").unwrap();
        let part = json!({"functionCall":{"name":"read_file","args":{"path":"one"}},"thoughtSignature":"opaque-signature"});
        let original = part.clone();
        let model = Model::new(move |round, _| {
            let response = if round == 0 {
                json!({"candidates":[{"content":{"role":"model","parts":[
                    {"text":"hidden-thought","thought":true,"thoughtSignature":"reasoning-signature"},part.clone()
                ]},"finishReason":"STOP"}]})
            } else {
                json!({"candidates":[{"content":{"role":"model","parts":[{"text":"done"}]},"finishReason":"STOP"}]})
            };
            if stream {
                Reply::Sse(vec![response])
            } else {
                Reply::Json(response)
            }
        });
        config(root.path(), &model, "gemini", stream, json!([]));
        let output = run(
            root.path(),
            &["sdk"],
            Some(json!({"type":"user","content":"Read one"})),
            |_| {},
        );
        assert!(output.status.success());
        let requests = model.requests();
        assert_eq!(requests.len(), 2);
        let replay = requests[1]["contents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "model")
            .unwrap();
        assert_eq!(
            replay["parts"][1], original,
            "signature and part boundary must survive"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("\"delta\":\"hidden-thought"));
        let session_file = fs::read_dir(root.path().join(".atlas/sessions"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .unwrap();
        let session: Value = serde_json::from_slice(&fs::read(&session_file).unwrap()).unwrap();
        let id = session["id"].as_str().unwrap();
        let output = run(
            root.path(),
            &["sdk", "--resume", id],
            Some(json!({"type":"user","content":"Continue"})),
            |_| {},
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let requests = model.requests();
        let replay = requests[2]["contents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "model")
            .unwrap();
        assert_eq!(replay["parts"][1], original);
    }
}

#[cfg(unix)]
#[test]
fn ultra_review_executes_only_read_only_pr_queries_and_keeps_explicit_denials() {
    use std::os::unix::fs::PermissionsExt;
    for behavior in ["default", "allow", "deny", "ask"] {
        let root = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .arg(root.path())
            .status()
            .unwrap()
            .success());
        let bin = root.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(
            bin.join("gh"),
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> gh-calls\nprintf 'actual PR data'\n",
        )
        .unwrap();
        fs::set_permissions(bin.join("gh"), fs::Permissions::from_mode(0o755)).unwrap();
        let model = Model::new(|round, _| {
            Reply::Json(if round == 0 {
                json!({"choices":[{"message":{"role":"assistant","tool_calls":[
                tool("run_command",json!({"command":"gh pr view 6"}),"view"),
                tool("run_command",json!({"command":"gh pr diff 6"}),"diff"),
                tool("run_command",json!({"command":"gh pr diff 6 > forbidden"}),"redirect"),
                tool("write_file",json!({"path":"forbidden","content":"bad"}),"write")
            ]},"finish_reason":"tool_calls"}]})
            } else {
                done()
            })
        });
        config(
            root.path(),
            &model,
            "openai-compatible",
            false,
            if behavior != "default" {
                json!([{"behavior":behavior,"rule":"Bash"}])
            } else {
                json!([])
            },
        );
        let output = run(root.path(), &["ultra-review", "6"], None, |command| {
            let paths = std::iter::once(bin.clone())
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
                .collect::<Vec<_>>();
            command.env("PATH", std::env::join_paths(paths).unwrap());
        });
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let requests = model.requests();
        let results = requests[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "tool")
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 4);
        if matches!(behavior, "deny" | "ask") {
            assert!(!root.path().join("gh-calls").exists());
        } else {
            assert_eq!(
                fs::read_to_string(root.path().join("gh-calls")).unwrap(),
                "pr view 6\npr diff 6\n"
            );
            assert!(results[0]["content"]
                .as_str()
                .unwrap()
                .contains("actual PR data"));
            assert!(results[1]["content"]
                .as_str()
                .unwrap()
                .contains("actual PR data"));
        }
        assert!(!root.path().join("forbidden").exists());
    }
}
