//! Original src/utils/jsonRead.ts::stripBOM, linked directly to real Rust production code.
#[path = "../json_read.rs"]
mod json_read;
use std::io::{self, Read};

const SOURCE_ID: &str = "src/utils/jsonRead.ts:FunctionDeclaration:14-16:occurrence-1";

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut request = String::new();
    io::stdin().read_to_string(&mut request)?;
    let payload: serde_json::Value = serde_json::from_str(&request)?;
    let case_id = payload["id"].as_str().ok_or("missing golden case ID")?;
    let content = payload["input"]["content"]
        .as_str()
        .ok_or("missing content")?;
    let result = json_read::strip_bom(content);
    let observed = serde_json::json!({
        "schema": "source-rust-observation/v1",
        "case_id": case_id,
        "executed_source_ids": [SOURCE_ID],
        "outcome": {"parsed": result},
        "errors": [],
        "state": {},
        "calls": ["stripBOM"],
        "permissions": [],
        "events": ["started", "finished"],
        "side_effects": [],
        "persistence": {},
        "cancellation": {"cancelled": false}
    });
    println!("{}", serde_json::to_string(&observed)?);
    Ok(())
}
