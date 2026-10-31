//! Original 2.1.88 multi-step circular-buffer behavior against production Rust.
#[path = "../circular_buffer.rs"]
mod circular_buffer;

use circular_buffer::CircularBuffer;
use serde_json::{json, Value};
use std::io::{self, Read};

const SOURCE_IDS: [&str; 7] = [
    "src/utils/CircularBuffer.ts:Constructor:10-12:occurrence-1",
    "src/utils/CircularBuffer.ts:MethodDeclaration:18-24:occurrence-1",
    "src/utils/CircularBuffer.ts:MethodDeclaration:29-33:occurrence-1",
    "src/utils/CircularBuffer.ts:MethodDeclaration:39-50:occurrence-1",
    "src/utils/CircularBuffer.ts:MethodDeclaration:55-67:occurrence-1",
    "src/utils/CircularBuffer.ts:MethodDeclaration:72-76:occurrence-1",
    "src/utils/CircularBuffer.ts:MethodDeclaration:81-83:occurrence-1",
];

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
    let case_id = request["id"].as_str().ok_or("missing case ID")?;
    let payload = &request["input"]["payload"];
    let capacity = payload["capacity"].as_u64().ok_or("missing capacity")?;
    let capacity = usize::try_from(capacity)?;
    let mut buffer: CircularBuffer<Value> = CircularBuffer::new(capacity)?;
    let steps = payload["steps"].as_array().ok_or("missing steps")?;
    let mut used = [true, false, false, false, true, false, true];
    let mut observed = Vec::with_capacity(steps.len());
    for step in steps {
        let op = step["op"].as_str().ok_or("missing operation")?;
        let value: Value = match op {
            "add" => {
                used[1] = true;
                buffer.add(step["value"].clone());
                Value::Null
            }
            "addAll" => {
                used[2] = true;
                let values = step["values"].as_array().ok_or("missing values")?;
                if !values.is_empty() {
                    used[1] = true;
                }
                buffer.add_all(values.iter().cloned());
                Value::Null
            }
            "getRecent" => {
                used[3] = true;
                let count = step["count"].as_u64().ok_or("missing count")?;
                json!(buffer.get_recent(usize::try_from(count)?))
            }
            "toArray" => json!(buffer.to_array()),
            "length" => json!(buffer.len()),
            "clear" => {
                used[5] = true;
                buffer.clear();
                Value::Null
            }
            _ => return Err("unknown original circular buffer operation".into()),
        };
        observed.push(json!({"op": op, "value": value,
                             "snapshot": buffer.to_array(),
                             "length": buffer.len()}));
    }
    let ids: Vec<&str> = SOURCE_IDS
        .iter()
        .enumerate()
        .filter_map(|(i, source_id)| used[i].then_some(*source_id))
        .collect();
    let response = json!({"schema":"source-rust-observation/v1",
        "case_id":case_id,"executed_source_ids":ids,
        "outcome":{"parsed":observed},"errors":[],
        "state":{},"calls":["CircularBuffer"],"permissions":[],
        "events":["started","finished"],"side_effects":[],
        "persistence":{},"cancellation":{"cancelled":false}});
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}
