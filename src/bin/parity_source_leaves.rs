//! Paired golden bridge: always invoke the real Rust production modules.
#[path = "../task_guards.rs"]
mod task_guards;
#[path = "../control_message_compat.rs"]
mod control_message_compat;
#[path = "../compat_helpers.rs"]
mod compat_helpers;

use std::io::{self, Read};

const TASK_ID: &str = "src/tasks/LocalShellTask/guards.ts:FunctionDeclaration:34-41:occurrence-1";
const CONTROL_ID: &str = "src/utils/controlMessageCompat.ts:FunctionDeclaration:13-32:occurrence-1";
const BASH_ID: &str = "src/tools/BashTool/commentLabel.ts:FunctionDeclaration:8-13:occurrence-1";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let case: serde_json::Value = serde_json::from_str(&input)?;
    let case_id = case["id"].as_str().ok_or("missing original case ID")?;
    let operation = case["input"]["operation"]
        .as_str()
        .ok_or("missing original function")?;
    let value = &case["input"]["value"];
    let (parsed, source_id) = match operation {
        "isLocalShellTask" => (
            serde_json::json!(task_guards::is_local_shell_task(value)),
            TASK_ID,
        ),
        "normalizeControlMessageKeys" => (
            control_message_compat::normalize_control_message_keys(value.clone()),
            CONTROL_ID,
        ),
        "extractBashCommentLabel" => {
            let source = value.as_str().ok_or("Bash label input must be a string")?;
            (
                serde_json::json!(compat_helpers::extract_bash_comment_label(source)),
                BASH_ID,
            )
        }
        _ => return Err("unknown original source function".into()),
    };
    let result = serde_json::json!({
        "schema": "source-rust-observation/v1",
        "case_id": case_id,
        "executed_source_ids": [source_id],
        "outcome": {"parsed": parsed},
        "errors": [],
        "state": {},
        "calls": [operation],
        "permissions": [],
        "events": ["started", "finished"],
        "side_effects": [],
        "persistence": {},
        "cancellation": {"cancelled": false}
    });
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
