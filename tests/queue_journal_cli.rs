use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn sdk_queue_journals_real_mutations_and_keeps_text_private() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        serde_json::json!({
            "default_provider":"local",
            "providers":[{"id":"local","kind":"openai-compatible",
                "base_url":"http://127.0.0.1:1/v1","model":"local"}]
        })
        .to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = [
        serde_json::json!({"type":"user","queued":true,"content":"first","uuid":"u1"}),
        serde_json::json!({"type":"user","queued":true,"content":"second","uuid":"u2"}),
        serde_json::json!({"type":"control","action":"queue_dequeue","request_id":"pop"}),
        serde_json::json!({"type":"control","action":"queue_remove_identity",
            "request_id":"remove","identities":[2]}),
    ]
    .iter()
    .map(|x| x.to_string())
    .collect::<Vec<_>>()
    .join("\n")
        + "\n";
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let replies = stdout
        .lines()
        .map(|x| serde_json::from_str::<serde_json::Value>(x).unwrap())
        .collect::<Vec<_>>();
    let id = replies[0]["session_id"].as_str().unwrap();
    assert_eq!(replies[1]["identity"], 1);
    assert_eq!(replies[2]["identity"], 2);
    assert_eq!(replies[3]["result"]["command"]["identity"], 1);
    assert_eq!(replies[4]["result"]["removed"], 1);
    let path = root.path().join(".atlas/queue").join(format!("{id}.jsonl"));
    let data = fs::read_to_string(path).unwrap();
    let events = data
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .map(|e| e["operation"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["enqueue", "enqueue", "dequeue", "remove"]
    );
    assert_eq!(events[0]["content"], "first");
    assert_eq!(events[1]["content"], "second");
    assert!(events.iter().all(|event| event["sessionId"] == id));
    assert!(!data.contains("ATLAS_CONFIG"));
}
#[cfg(unix)]
#[test]
fn sdk_queue_journal_rejects_symlink_escape_before_reading_commands() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".atlas")).unwrap();
    symlink(outside.path(), root.path().join(".atlas/queue")).unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        serde_json::json!({
            "default_provider":"local",
            "providers":[{"id":"local","kind":"openai-compatible",
                "base_url":"http://127.0.0.1:1/v1","model":"local"}]
        })
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Queue journal escapes workspace"));
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}
