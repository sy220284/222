//! Real original user-prompt keyword functions from production Rust module.
#[path = "../source_user_keywords.rs"]
mod source_user_keywords;
use serde_json::{json, Value};
use std::io::{self, Read};
const NEGATIVE: &str = "src/utils/userPromptKeywords.ts:FunctionDeclaration:4-11:occurrence-1";
const CONTINUE: &str = "src/utils/userPromptKeywords.ts:FunctionDeclaration:16-27:occurrence-1";
fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;
    let request: Value = serde_json::from_str(&data)?;
    let id = request["id"].as_str().ok_or("missing test case ID")?;
    let operation = request["input"]["operation"]
        .as_str()
        .ok_or("missing original operation")?;
    let text = request["input"]["payload"]
        .as_str()
        .ok_or("keyword test input must be text")?;
    let (value, source_id) = match operation {
        "matchesNegativeKeyword" => (
            source_user_keywords::matches_negative_keyword(text),
            NEGATIVE,
        ),
        "matchesKeepGoingKeyword" => (
            source_user_keywords::matches_keep_going_keyword(text),
            CONTINUE,
        ),
        _ => return Err("unknown original keyword export".into()),
    };
    let observed = json!({
        "schema":"source-rust-observation/v1", "case_id":id,
        "executed_source_ids":[source_id], "outcome":{"parsed":value},
        "errors":[], "state":{}, "calls":[operation], "permissions":[],
        "events":["started","finished"], "side_effects":[],
        "persistence":{}, "cancellation":{"cancelled":false}
    });
    println!("{}", serde_json::to_string(&observed)?);
    Ok(())
}
