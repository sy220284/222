//! Execute the real stateful command lifecycle implementation as a pairable binary.
#[path = "../command_lifecycle.rs"]
mod command_lifecycle;

use command_lifecycle::{set_command_lifecycle_listener, notify_command_lifecycle, State};
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

const SET_ID: &str = "src/utils/commandLifecycle.ts:FunctionDeclaration:10-14:occurrence-1";
const NOTIFY_ID: &str = "src/utils/commandLifecycle.ts:FunctionDeclaration:16-21:occurrence-1";

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
    let steps = request["input"]["payload"]["steps"]
        .as_array()
        .ok_or("missing original lifecycle action sequence")?;
    let events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let mut used_set = false;
    let mut used_notify = false;
    set_command_lifecycle_listener(None);
    for step in steps {
        let op = step["op"].as_str().ok_or("invalid lifecycle action")?;
        match op {
            "set" => {
                used_set = true;
                let listener = step["listener"].as_str().ok_or("missing listener tag")?.to_owned();
                let sink = Arc::clone(&events);
                set_command_lifecycle_listener(Some(Arc::new(move |uuid, state| {
                    let state_label = match state {
                        State::Started => "started",
                        State::Completed => "completed",
                    };
                    sink.lock().unwrap().push(serde_json::json!({
                        "listener": listener, "uuid": uuid, "state": state_label
                    }));
                })));
            }
            "clear" => {
                used_set = true;
                set_command_lifecycle_listener(None);
            }
            "notify" => {
                used_notify = true;
                let uuid = step["uuid"].as_str().ok_or("missing UUID")?;
                let state = match step["state"].as_str() {
                    Some("started") => State::Started,
                    Some("completed") => State::Completed,
                    _ => return Err("invalid original lifecycle state".into()),
                };
                notify_command_lifecycle(uuid, state);
            }
            _ => return Err("unknown lifecycle action".into()),
        }
    }
    set_command_lifecycle_listener(None);
    let ids: Vec<&str> = [used_set.then_some(SET_ID), used_notify.then_some(NOTIFY_ID)]
        .into_iter()
        .flatten()
        .collect();
    let result = events.lock().unwrap().clone();
    let observed = serde_json::json!({
        "schema": "source-rust-observation/v1",
        "case_id": case_id,
        "executed_source_ids": ids,
        "outcome": {"parsed": result},
        "errors": [],
        "state": {},
        "calls": ["lifecycle_actions"],
        "permissions": [],
        "events": ["started", "finished"],
        "side_effects": [],
        "persistence": {},
        "cancellation": {"cancelled": false}
    });
    println!("{}", serde_json::to_string(&observed)?);
    Ok(())
}
