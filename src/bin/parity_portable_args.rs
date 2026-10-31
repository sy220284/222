//! Original peerAddress and cliArgs leaf function parity against real Rust modules.
#[path = "../cli_args.rs"]
mod cli_args;
#[path = "../peer_address.rs"]
mod peer_address;
use serde_json::{json, Value};
use std::io::{self, Read};

const PEER: &str = "src/utils/peerAddress.ts:FunctionDeclaration:8-21:occurrence-1";
const EAGER: &str = "src/utils/cliArgs.ts:FunctionDeclaration:13-29:occurrence-1";
const DOUBLE: &str = "src/utils/cliArgs.ts:FunctionDeclaration:49-60:occurrence-1";

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
        .ok_or("missing operation")?;
    let input = &request["input"]["payload"];
    let (value, source_id) = match operation {
        "parseAddress" => {
            let input = input.as_str().ok_or("expected peer address string")?;
            let (kind, target) = peer_address::parse_peer_address(input);
            let scheme = match kind {
                peer_address::PeerScheme::Uds => "uds",
                peer_address::PeerScheme::Bridge => "bridge",
                peer_address::PeerScheme::Other => "other",
            };
            (json!({"scheme":scheme,"target":target}), PEER)
        }
        "eagerParseCliFlag" => {
            let flag = input["flag"].as_str().ok_or("missing flag name")?;
            let args: Vec<String> = serde_json::from_value(input["argv"].clone())?;
            (json!(cli_args::eager_parse_cli_flag(flag, &args)), EAGER)
        }
        "extractArgsAfterDoubleDash" => {
            let command = input["command"].as_str().ok_or("missing command")?;
            let args: Vec<String> = serde_json::from_value(input["args"].clone())?;
            let (command, args) = cli_args::extract_args_after_double_dash(command, &args);
            (json!({"command":command,"args":args}), DOUBLE)
        }
        _ => return Err("unknown original exported function".into()),
    };
    let observed = json!({"schema":"source-rust-observation/v1",
        "case_id":id,"executed_source_ids":[source_id],
        "outcome":{"parsed":value},"errors":[],"state":{},
        "calls":[operation],"permissions":[],"events":["started","finished"],
        "side_effects":[],"persistence":{},"cancellation":{"cancelled":false}});
    println!("{}", serde_json::to_string(&observed)?);
    Ok(())
}
