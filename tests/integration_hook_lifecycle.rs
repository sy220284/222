use serde_json::{json, Value};
use std::{fs, io::{Read, Write}, net::{TcpListener, TcpStream}, process::Command, thread, time::Duration};

fn read_request(stream: &mut TcpStream) -> String {
    stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let mut data = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 { break; }
        data.extend_from_slice(&chunk[..count]);
        if let Some(index) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&data[..index]);
            let length = headers.lines().find_map(|line| {
                line.to_ascii_lowercase().strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            }).unwrap_or(0);
            if data.len() >= index + 4 + length { break; }
        }
    }
    String::from_utf8(data).unwrap()
}

#[cfg(unix)]
#[test]
fn hook_lifecycle_uses_executed_tools_and_structured_effective_input() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("secret.txt"), "DO_NOT_EXPOSE_PREHOOK\n").unwrap();
    let pre_payload = root.path().join("pre-payload.json");
    let post_success = root.path().join("post-success.json");
    let post_failure = root.path().join("post-failure.json");
    let blocked_post = root.path().join("blocked-post.txt");
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
                        {"id":"blocked-read","type":"function","function":{
                            "name":"read_file",
                            "arguments":json!({"path":"secret.txt"}).to_string()}},
                        {"id":"rewritten-write","type":"function","function":{
                            "name":"write_file",
                            "arguments":json!({"path":"original.txt",
                                "content":"original"}).to_string()}},
                        {"id":"failed-edit","type":"function","function":{
                            "name":"edit_file",
                            "arguments":json!({"path":"absent.txt",
                                "old_string":"x","new_string":"y"}).to_string()}},
                        {"id":"healthy-sibling","type":"function","function":{
                            "name":"list_files","arguments":"{}"}}
                    ]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"All tool outcomes received"},"finish_reason":"stop"}]})
            };
            let body = response.to_string();
            write!(conn, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            conn.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("config.json");
    fs::write(&config, json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"model",
        "base_url":format!("http://{address}/v1")
    }],"hooks":[
        {"event":"PreToolUse","matcher":"^read_file$",
            "command":"printf '{\"decision\":\"block\",\"reason\":\"read denied\"}'"},
        {"event":"PreToolUse","matcher":"^write_file$",
            "command":"printf '{\"hookSpecificOutput\":{\"updatedInput\":{\"path\":\"rewritten.txt\",\"content\":\"effective content\"}}}'"},
        {"event":"PreToolUse","matcher":"^write_file$",
            "command":format!("cat > '{}'", pre_payload.display()),
            "timeout_seconds":3},
        {"event":"PostToolUseFailure","matcher":"^read_file$",
            "command":format!("printf unexpected > '{}'", blocked_post.display())},
        {"event":"PostToolUseFailure","matcher":"^edit_file$",
            "command":format!("cat > '{}'", post_failure.display()),
            "timeout_seconds":3},
        {"event":"PostToolUse","matcher":"^write_file$",
            "command":format!("cat > '{}'", post_success.display()),
            "timeout_seconds":3}
    ]}).to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(),
            "--approval", "full", "run", "Exercise hook lifecycle"])
        .env("ATLAS_CONFIG", &config).output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("All tool outcomes received"));
    assert_eq!(requests.len(), 2);
    let followup = &requests[1];
    for id in ["blocked-read","rewritten-write","failed-edit","healthy-sibling"] {
        assert!(followup.contains(id), "missing {id}: {followup}");
    }
    assert!(followup.contains("PreToolUse hook blocked"), "{followup}");
    assert!(!followup.contains("PostToolUse hook failed"), "{followup}");
    assert!(!followup.contains("Hook timed out"), "{followup}");
    assert!(!followup.contains("DO_NOT_EXPOSE_PREHOOK"), "{followup}");
    assert!(!blocked_post.exists(), "a skipped tool ran PostToolUseFailure");
    assert!(!root.path().join("original.txt").exists());
    assert_eq!(fs::read_to_string(root.path().join("rewritten.txt")).unwrap(),
        "effective content");
    let pre: Value = serde_json::from_str(&fs::read_to_string(pre_payload).unwrap()).unwrap();
    assert_eq!(pre["hook_event_name"], "PreToolUse");
    assert_eq!(pre["tool_use_id"], "rewritten-write");
    assert_eq!(pre["tool_input"]["path"], "original.txt");
    assert!(pre.get("input").is_none());
    let post: Value = serde_json::from_str(&fs::read_to_string(post_success).unwrap()).unwrap();
    assert_eq!(post["hook_event_name"], "PostToolUse");
    assert_eq!(post["tool_use_id"], "rewritten-write");
    assert_eq!(post["tool_input"]["path"], "rewritten.txt");
    assert_eq!(post["tool_input"]["content"], "effective content");
    assert!(post["tool_response"].is_string());
    let failed: Value = serde_json::from_str(&fs::read_to_string(post_failure).unwrap()).unwrap();
    assert_eq!(failed["hook_event_name"], "PostToolUseFailure");
    assert_eq!(failed["tool_use_id"], "failed-edit");
    assert_eq!(failed["tool_input"]["path"], "absent.txt");
    assert!(!failed["error"].as_str().unwrap().is_empty());
    assert!(!failed["error"].as_str().unwrap().starts_with("ERROR:"));
    assert_eq!(failed["is_interrupt"], false);
    assert!(failed.get("tool_response").is_none());
}
