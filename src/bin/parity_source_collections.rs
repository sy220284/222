//! Tests ORIGINAL Set primitives and Object.groupBy via real production Rust code.
#[path = "../collections.rs"]
mod collections;
use indexmap::IndexSet;
use serde_json::{json, Value};
use std::{collections::HashMap, io::{self, Read}};

const IDS: [(&str,&str);5] = [
    ("difference", "src/utils/set.ts:FunctionDeclaration:4-12:occurrence-1"),
    ("intersects", "src/utils/set.ts:FunctionDeclaration:17-27:occurrence-1"),
    ("every", "src/utils/set.ts:FunctionDeclaration:32-39:occurrence-1"),
    ("union", "src/utils/set.ts:FunctionDeclaration:44-53:occurrence-1"),
    ("objectGroupBy", "src/utils/objectGroupBy.ts:FunctionDeclaration:4-18:occurrence-1"),
];
fn primitive_key(value: &Value) -> Result<String, &'static str> {
    if value.is_object() || value.is_array() {
        return Err("original Set object identity needs dedicated fixture");
    }
    Ok(serde_json::to_string(value).expect("JSON primitive encodes"))
}
fn ordered_keys(value: &Value) -> Result<(IndexSet<String>,HashMap<String,Value>),Box<dyn std::error::Error>> {
    let values = value.as_array().ok_or("set operand must be array")?;
    let mut set = IndexSet::new();
    let mut decoded = HashMap::new();
    for value in values {
        let key = primitive_key(value)?;
        set.insert(key.clone());
        decoded.entry(key).or_insert_with(|| value.clone());
    }
    Ok((set,decoded))
}
fn group_key(value: &Value,index: usize,mode: &str) -> Result<String,&'static str> {
    Ok(match mode {
        "parity" => if value.as_i64().ok_or("parity expects integer")? % 2 == 0 {
            "even".into()
        } else { "odd".into() },
        "initial" => value.as_str().ok_or("initial expects string")?
            .chars().next().map(|c| c.to_string()).unwrap_or_default(),
        "identity" => value.as_str().ok_or("identity expects string")?.to_owned(),
        "index-mod3" => (index % 3).to_string(),
        "all" => "__proto__".to_owned(),
        "type" => match value {
            Value::Null => "null".into(),
            Value::Bool(_) => "boolean".into(),
            Value::Number(_) => "number".into(),
            Value::String(_) => "string".into(),
            _ => "object".into(),
        },
        _ => return Err("unknown original group selector"),
    })
}
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
    let op = request["input"]["operation"].as_str().ok_or("missing operation")?;
    let payload = &request["input"]["payload"];
    let source_id = IDS.iter().find(|(name,_)| *name == op)
        .ok_or("unknown original collection export")?.1;
    let result = match op {
        "difference" | "intersects" | "every" | "union" => {
            let (a, originals_a) = ordered_keys(&payload["a"])?;
            let (b, originals_b) = ordered_keys(&payload["b"])?;
            match op {
                "difference" => json!(collections::difference(&a,&b)
                    .iter().map(|key| originals_a[key].clone()).collect::<Vec<_>>()),
                "intersects" => json!(collections::intersects(&a,&b)),
                "every" => json!(collections::every(&a,&b)),
                "union" => json!(collections::union(&a,&b)
                    .iter().map(|key| originals_a.get(key).or_else(|| originals_b.get(key))
                        .expect("union key exists").clone()).collect::<Vec<_>>()),
                _ => unreachable!(),
            }
        }
        "objectGroupBy" => {
            let values = payload["values"].as_array().ok_or("missing values")?;
            let mode = payload["mode"].as_str().ok_or("missing grouping mode")?;
            let grouped = collections::object_group_by(values.iter().cloned(),
                |item,index| group_key(item,index,mode).expect("registered selector is valid"));
            json!(grouped)
        }
        _ => return Err("unknown original collection function".into()),
    };
    let observation = json!({
        "schema":"source-rust-observation/v1", "case_id":case_id,
        "executed_source_ids":[source_id], "outcome":{"parsed":result},
        "errors":[], "state":{}, "calls":[op], "permissions":[],
        "events":["started","finished"], "side_effects":[],
        "persistence":{}, "cancellation":{"cancelled":false}
    });
    println!("{}", serde_json::to_string(&observation)?);
    Ok(())
}
