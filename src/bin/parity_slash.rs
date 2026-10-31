//! Independent runtime parity adapter: invokes the real production module.
//! The observed function is compiled from src/slash_command.rs (not duplicated).
#[path = "../slash_command.rs"]
mod slash_command;
use std::io::{self, Read};

const SOURCE_ID: &str =
    "src/utils/slashCommandParsing.ts:FunctionDeclaration:25-60:occurrence-1";

fn main() {
    if let Err(e) = real_main() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), Box<dyn std::error::Error>> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;
    let request: serde_json::Value = serde_json::from_str(&data)?;
    let case_id = request["id"]
        .as_str()
        .ok_or("missing case id")?;
    let command = request["input"]["command"]
        .as_str()
        .ok_or("missing slash command input")?;
    let parsed = slash_command::parse_slash_command(command).map(|value| {
        serde_json::json!({
            "commandName": value.command_name,
            "args": value.args,
            "isMcp": value.is_mcp,
        })
    });
    let observation = serde_json::json!({
        "schema": "source-rust-observation/v1",
        "case_id": case_id,
        "executed_source_ids": [SOURCE_ID],
        "outcome": { "parsed": parsed },
        "errors": [],
        "state": {},
        "calls": ["parseSlashCommand"],
        "permissions": [],
        "events": ["started", "finished"],
        "side_effects": [],
        "persistence": {},
        "cancellation": { "cancelled": false },
    });
    println!("{}", serde_json::to_string(&observation)?);
    Ok(())
}
