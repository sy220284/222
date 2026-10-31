//! Real process integration check for listener replacement and event order.
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn original_lifecycle_listener_replacement_matches_real_rust_process() {
    let fixture: Value =
        serde_json::from_str(include_str!("source_parity/golden/command_lifecycle.json")).unwrap();
    let example = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "listener-replaced")
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_parity_command_lifecycle"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({
        "id": example["id"],
        "input": {"payload": example["input"]}
    });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual["outcome"]["parsed"], example["expected"]);
    assert_eq!(actual["executed_source_ids"], example["source_ids"]);
}
