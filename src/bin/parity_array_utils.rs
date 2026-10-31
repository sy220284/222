//! Frozen original src/utils/array.ts contract against actual Rust production.
#[path = "../array_utils.rs"]
mod array_utils;
use std::io::{self, Read};

const INTERSPERSE: &str = "src/utils/array.ts:FunctionDeclaration:1-3:occurrence-1";
const CALLBACK: &str = "src/utils/array.ts:ArrowFunction:2-2:occurrence-1";
const COUNT: &str = "src/utils/array.ts:FunctionDeclaration:5-9:occurrence-1";
const UNIQ: &str = "src/utils/array.ts:FunctionDeclaration:11-13:occurrence-1";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: serde_json::Value = serde_json::from_str(&input)?;
    let case_id = request["id"].as_str().ok_or("missing golden ID")?;
    let payload = &request["input"]["payload"];
    let values: Vec<i64> = serde_json::from_value(payload["values"].clone())?;
    let op = payload["operation"].as_str().ok_or("missing array operation")?;
    let (parsed, source_ids): (serde_json::Value, Vec<&str>) = match op {
        "intersperse" => {
            if payload["mode"].as_str() != Some("negative-index") {
                return Err("unsupported separator test recipe".into());
            }
            let observed = array_utils::intersperse(&values, |index| -(index as i64));
            let ids = if values.len() > 1 {
                vec![INTERSPERSE, CALLBACK]
            } else {
                vec![INTERSPERSE]
            };
            (serde_json::json!(observed), ids)
        }
        "count" => {
            let mode = payload["mode"].as_str().ok_or("missing count predicate")?;
            let result = match mode {
                "positive" => array_utils::count(&values, |value| *value > 0),
                "even" => array_utils::count(&values, |value| value % 2 == 0),
                _ => return Err("unknown count predicate".into())
            };
            (serde_json::json!(result), vec![COUNT])
        }
        "uniq" => (
            serde_json::json!(array_utils::uniq(values)),
            vec![UNIQ]
        ),
        _ => return Err("unknown original array export".into()),
    };
    let output = serde_json::json!({
        "schema":"source-rust-observation/v1",
        "case_id":case_id,
        "executed_source_ids":source_ids,
        "outcome":{"parsed":parsed},
        "errors":[],
        "state":{},
        "calls":[op],
        "permissions":[],
        "events":["started","finished"],
        "side_effects":[],
        "persistence":{},
        "cancellation":{"cancelled":false}
    });
    println!("{}",serde_json::to_string(&output)?);
    Ok(())
}
