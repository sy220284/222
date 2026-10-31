//! Real Rust timeout implementation receives per-case synthetic env only.
#[path = "../source_timeouts.rs"]
mod source_timeouts;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{self, Read},
};

const DEFAULT: &str = "src/utils/timeouts.ts:FunctionDeclaration:12-21:occurrence-1";
const MAX: &str = "src/utils/timeouts.ts:FunctionDeclaration:28-39:occurrence-1";

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
    let op = request["input"]["operation"]
        .as_str()
        .ok_or("missing operation")?;
    let obj = request["input"]["payload"]
        .as_object()
        .ok_or("missing env object")?;
    let env: HashMap<String, String> = obj
        .iter()
        .map(|(key, value)| {
            Ok((
                key.to_owned(),
                value.as_str().ok_or("env value must be text")?.to_owned(),
            ))
        })
        .collect::<Result<_, &str>>()?;
    let (result, ids): (u64, Vec<&str>) = match op {
        "getDefaultBashTimeoutMs" => (
            source_timeouts::default_bash_timeout_ms(&env),
            vec![DEFAULT],
        ),
        "getMaxBashTimeoutMs" => (
            source_timeouts::max_bash_timeout_ms(&env),
            vec![DEFAULT, MAX],
        ),
        _ => return Err("unknown original timeout export".into()),
    };
    let observation = json!({
        "schema":"source-rust-observation/v1", "case_id":case_id,
        "executed_source_ids":ids, "outcome":{"parsed":result},
        "errors":[], "state":{}, "calls":[op], "permissions":[],
        "events":["started","finished"], "side_effects":[],
        "persistence":{}, "cancellation":{"cancelled":false}
    });
    println!("{}", serde_json::to_string(&observation)?);
    Ok(())
}
