use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn mcp_chat_all_toggle_uses_live_statuses_and_persists_disabled_state() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider":"mock",
            "providers":[{"id":"mock","kind":"openai-compatible",
                "model":"test-model","base_url":"http://127.0.0.1:1234/v1"}],
            "mcp_servers":[{"id":"fixture", "command":"nonexistent-mcp-parity-fixture-xyz"}]
        })
        .to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
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
        .write_all(
            b"/mcp disable all\n/mcp disable all\n/mcp disable missing\n/mcp status\n/exit\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Disabled 1 MCP server(s)"), "{text}");
    assert!(
        text.contains("All MCP servers are already disabled"),
        "{text}"
    );
    assert!(text.contains(r#"MCP server "missing" not found"#), "{text}");
    assert!(text.contains("\"status\": \"disabled\""), "{text}");
    let disabled: Value =
        serde_json::from_slice(&fs::read(config.with_file_name("mcp-disabled.json")).unwrap())
            .unwrap();
    assert!(
        disabled.as_array().unwrap().iter().any(|v| v == "fixture"),
        "{disabled}"
    );
}
