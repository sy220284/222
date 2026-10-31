//! Frozen behavior from the ORIGINAL 2.1.88 parseSlashCommand function.
//! This exercises the real production Rust function and its runtime adapter.
#[path = "../src/slash_command.rs"]
mod slash_command;

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn fixture() -> Value {
    serde_json::from_str(include_str!("source_parity/golden/slash_command.json"))
        .expect("original source golden corpus must be valid JSON")
}

#[test]
fn original_2_1_88_golden_cases_match_the_rust_parser() {
    let corpus = fixture();
    assert_eq!(corpus["schema"], "source-golden/v1");
    assert_eq!(corpus["source_path"], "src/utils/slashCommandParsing.ts");
    for case in corpus["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let output = slash_command::parse_slash_command(input).map(|value| {
            json!({
                "commandName": value.command_name,
                "args": value.args,
                "isMcp": value.is_mcp
            })
        });
        let observed = serde_json::to_value(output).unwrap();
        assert_eq!(
            observed, case["expected"],
            "original golden case: {}", case["id"]
        );
    }
}

#[test]
fn paired_test_binary_executes_real_rust_parser_for_all_golden_inputs() {
    let corpus = fixture();
    for case in corpus["cases"].as_array().unwrap() {
        let mut child = Command::new(env!("CARGO_BIN_EXE_parity_slash"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("real Rust parity binary must execute");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                json!({"id":case["id"],"input":{"command":case["input"]}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "case={} stderr={}",
            case["id"],
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            result["outcome"]["parsed"], case["expected"],
            "case {}", case["id"]
        );
        assert_eq!(
            result["executed_source_ids"][0],
            "src/utils/slashCommandParsing.ts:FunctionDeclaration:25-60:occurrence-1"
        );
    }
}
