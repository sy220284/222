//! Original 2.1.88 message discrimination and sandbox text cleanup.
#[path = "../message_predicates.rs"]
mod message_predicates;
#[path = "../sandbox_ui_utils.rs"]
mod sandbox_ui_utils;

use serde_json::{json, Value};
use std::io::{self, Read};

const PRED: &str = "src/utils/messagePredicates.ts:FunctionDeclaration:6-8:occurrence-1";
const SANDBOX: &str =
    "src/utils/sandbox/sandbox-ui-utils.ts:FunctionDeclaration:10-12:occurrence-1";

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;
    let request: Value = serde_json::from_str(&raw)?;
    let id = request["id"].as_str().ok_or("missing case ID")?;
    let operation = request["input"]["operation"]
        .as_str()
        .ok_or("missing original operation")?;
    let input = &request["input"]["payload"];
    let (result, source_id) = match operation {
        "isHumanTurn" => (json!(message_predicates::is_human_turn(input)), PRED),
        "removeSandboxViolationTags" => {
            let text = input.as_str().ok_or("sandbox input must be text")?;
            (
                json!(sandbox_ui_utils::remove_sandbox_violation_tags(text)),
                SANDBOX,
            )
        }
        _ => return Err("unrecognized original exported function".into()),
    };
    let observed = json!({"schema":"source-rust-observation/v1",
        "case_id":id,"executed_source_ids":[source_id],
        "outcome":{"parsed":result},"errors":[],"state":{},
        "calls":[operation],"permissions":[],"events":["started","finished"],
        "side_effects":[],"persistence":{},"cancellation":{"cancelled":false}});
    println!("{}", serde_json::to_string(&observed)?);
    Ok(())
}
