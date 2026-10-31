//! Real-process integration regression for the original circular buffer.
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn original_buffer_eviction_clear_and_reuse_matches_real_rust_process() {
    let fixture: Value =
        serde_json::from_str(include_str!("source_parity/golden/circular_buffer.json")).unwrap();
    let scenario = fixture["cases"].as_array().unwrap()
        .iter().find(|c| c["id"] == "clear-and-reuse").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_parity_circular_buffer"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    let request = json!({"id":scenario["id"],
                        "input":{"payload":scenario["input"]}});
    child.stdin.take().unwrap().write_all(request.to_string().as_bytes()).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let observed: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(observed["outcome"]["parsed"],scenario["expected"]);
    assert_eq!(observed["executed_source_ids"],scenario["source_ids"]);
}
