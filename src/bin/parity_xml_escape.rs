//! Paired original src/utils/xml.ts functions: call the real Rust implementation.
#[path = "../xml_escape.rs"]
mod xml_escape;

use std::io::{self, Read};

const TEXT_ID: &str = "src/utils/xml.ts:FunctionDeclaration:6-8:occurrence-1";
const ATTR_ID: &str = "src/utils/xml.ts:FunctionDeclaration:14-16:occurrence-1";

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: serde_json::Value = serde_json::from_str(&input)?;
    let case_id = request["id"].as_str().ok_or("missing case ID")?;
    let value = request["input"]["content"]
        .as_str()
        .ok_or("missing original input string")?;
    let operation = request["input"]["operation"]
        .as_str()
        .ok_or("missing original exported function")?;
    let (result, source_ids): (String, Vec<&str>) = match operation {
        "escapeXml" => (xml_escape::escape_xml(value), vec![TEXT_ID]),
        "escapeXmlAttr" => (xml_escape::escape_xml_attr(value), vec![TEXT_ID, ATTR_ID]),
        _ => return Err("unknown original XML function".into()),
    };
    let observed = serde_json::json!({
        "schema": "source-rust-observation/v1",
        "case_id": case_id,
        "executed_source_ids": source_ids,
        "outcome": {"parsed": result},
        "errors": [],
        "state": {},
        "calls": [operation],
        "permissions": [],
        "events": ["started", "finished"],
        "side_effects": [],
        "persistence": {},
        "cancellation": {"cancelled": false}
    });
    println!("{}", serde_json::to_string(&observed)?);
    Ok(())
}
