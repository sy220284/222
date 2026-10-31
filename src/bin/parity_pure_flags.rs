//! Actual Rust production functions from frozen original utility exports.
#[path = "../compat_helpers.rs"]
mod compat_helpers;
#[path = "../source_keyboard.rs"]
mod source_keyboard;
use serde_json::{json, Value};
use std::io::{self, Read};

const KEYBOARD: &str = "src/utils/keyboardShortcuts.ts:FunctionDeclaration:10-14:occurrence-1";
const WORKTREE: &str = "src/utils/worktreeModeEnabled.ts:FunctionDeclaration:9-11:occurrence-1";

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Value = serde_json::from_str(&input)?;
    let case_id = request["id"].as_str().ok_or("missing case ID")?;
    let operation = request["input"]["operation"]
        .as_str()
        .ok_or("missing operation")?;
    let (result, source_id) = match operation {
        "isMacosOptionChar" => {
            let text = request["input"]["payload"]
                .as_str()
                .ok_or("missing shortcut")?;
            (json!(source_keyboard::is_macos_option_char(text)), KEYBOARD)
        }
        "isWorktreeModeEnabled" => (json!(compat_helpers::is_worktree_mode_enabled()), WORKTREE),
        _ => return Err("unknown original export".into()),
    };
    let observation = json!({
        "schema":"source-rust-observation/v1", "case_id":case_id,
        "executed_source_ids":[source_id], "outcome":{"parsed":result},
        "errors":[], "state":{}, "calls":[operation], "permissions":[],
        "events":["started","finished"], "side_effects":[],
        "persistence":{}, "cancellation":{"cancelled":false}
    });
    println!("{}", serde_json::to_string(&observation)?);
    Ok(())
}
