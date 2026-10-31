use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn failed_sdk_initialize_cannot_leave_partial_agent_catalog() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "model":"test-model","base_url":"http://127.0.0.1:1234/v1"}]
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
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({
                "type":"control_request","request_id":"bad-init",
                "request":{"subtype":"initialize",
                    "agents":{"rogue":{"description":"Must never survive","prompt":"Test"}},
                    "jsonSchema":{"properties":{"foo":{"type":"string"}}}}
            })
        )
        .unwrap();
        writeln!(
            input,
            "{}",
            json!({
                "type":"control_request","request_id":"good-init",
                "request":{"subtype":"initialize"}
            })
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire = String::from_utf8_lossy(&output.stdout);
    let events = wire
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let bad = events
        .iter()
        .find(|event| {
            event["response"]["request_id"] == "bad-init" || event["request_id"] == "bad-init"
        })
        .expect("Missing first response");
    assert!(bad.to_string().contains("error"), "{bad}");
    let good = events
        .iter()
        .find(|event| {
            event["response"]["request_id"] == "good-init" || event["request_id"] == "good-init"
        })
        .expect("Missing second response");
    assert!(good.to_string().contains("success"), "{good}");
    let agents = good
        .pointer("/response/response/agents")
        .or_else(|| good.pointer("/response/agents"))
        .and_then(Value::as_array)
        .expect("SDK initialize must return an agent catalog");
    assert!(
        !agents.iter().any(|agent| agent["name"] == "rogue"),
        "{good}"
    );
}
