mod agent;
mod hooks;
mod worktree;
mod mcp;
mod config;
mod protocol;
mod provider;
mod session;
mod task;
mod task_graph;
mod task_events;
mod team_mailbox;
mod event_signal;
mod in_memory_mailbox;
mod team_registry;
mod tools;
mod notebook;
mod search;
mod lsp;
mod webfetch;
mod history;
mod plugins;
mod websearch;
mod sse;
mod background;
mod memory;
mod memory_age;
mod task_guards;
mod env_validation;
mod task_output_format;
mod compat_helpers;
mod runtime_contracts;
mod tool_names;
mod ndjson;
mod array_utils;
mod ink_constants;
mod command_lifecycle;
mod permission_rules;
mod reactive_store;
mod cleanup_registry;

macro_rules! print_json {
    ($value:expr)=>{{ println!("{}",crate::ndjson::stringify_value(&$value)); }};
}

use agent::Agent;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use config::{canonical_workspace, Config};
use plugins::PluginSet;
use session::Session;
use std::{
    io::{self, Read, Write},
    path::PathBuf,
};
use tools::Approval;
use serde_json::{json, Value};
use std::io::BufRead;

#[derive(Parser, Debug)]
#[command(name = "atlas-agent", version, about = "Standalone multi-model Rust coding agent")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Provider id from config.json (e.g. local, openai, gemini)
    #[arg(long, global = true)]
    provider: Option<String>,

    /// Override the selected provider's model id
    #[arg(long, global = true)]
    model: Option<String>,

    /// Root directory for file tools and shell execution
    #[arg(long, global = true, default_value = ".")]
    workspace: PathBuf,

    /// Read-only, interactive approval, or unrestricted tool execution
    #[arg(long, global = true, value_enum, default_value = "ask")]
    approval: ApprovalArg,

    /// Limit model/tool cycles per request
    #[arg(long, global = true, default_value_t = 24)]
    max_steps: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ApprovalArg {
    ReadOnly,
    Ask,
    Full,
}

impl From<ApprovalArg> for Approval {
    fn from(value: ApprovalArg) -> Self {
        match value {
            ApprovalArg::ReadOnly => Approval::ReadOnly,
            ApprovalArg::Ask => Approval::Ask,
            ApprovalArg::Full => Approval::Full,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a user-owned JSON config file with example providers
    Init,
    /// Show configured providers and active default without connecting to them
    Models,
    /// Execute a single prompt; pass '-' to read the prompt from stdin
    Run { prompt: String },
    /// Start an interactive conversation (optionally resume an earlier UUID)
    Chat {
        #[arg(long)]
        resume: Option<String>,
    },
    /// JSON-lines SDK mode for external programs (no proprietary transport)
    Sdk {
        #[arg(long)]
        resume: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Commands::Init = &cli.command {
        let path = Config::init()?;
        println!("Created {}", path.display());
        return Ok(());
    }
    let config = Config::load()?;
    if let Commands::Models = &cli.command {
        for profile in &config.providers {
            let default = if profile.id == config.default_provider { " (default)" } else { "" };
            println!("{}{}: {} / {} @ {}", profile.id, default, profile.kind, profile.model, profile.base_url);
        }
        return Ok(());
    }
    if cli.max_steps == 0 { bail!("--max-steps must be at least 1") }
    let root = canonical_workspace(&cli.workspace)?;
    let plugins = PluginSet::load(&root,&config.enabled_plugins)?;
    let profile = config.select(cli.provider.as_deref(), cli.model.as_deref())?;
    let interactive = matches!(&cli.command, Commands::Chat { .. });
    let session = match &cli.command {
        Commands::Chat { resume: Some(id) } | Commands::Sdk { resume: Some(id) } => Session::load(&root, id)?,
        _ => Session::fresh(),
    };
    let mut agent = Agent::new(
        profile, root, cli.approval.into(), interactive, cli.max_steps, session
    )?;
    agent.set_enabled_plugins(config.enabled_plugins.clone());
    agent.set_search_backend(config.search_backend.clone());
    let mut mcp_profiles = config.mcp_servers.clone();
    mcp_profiles.extend(plugins.mcp_servers);
    agent.attach_mcp(&mcp_profiles).await?;
    agent.attach_lsp(&config.lsp_servers).await?;
    let mut hooks = config.hooks.clone();
    hooks.extend(plugins.hooks);
    agent.attach_hooks(&hooks).await?;
    if matches!(&cli.command,Commands::Sdk { .. }) {
        agent.set_structured_output(true);
    }
    eprintln!("Session: {}", agent.session_id());
    eprintln!("Provider: {} / {}", agent.profile().id, agent.profile().model);
    let result = match &cli.command {
        Commands::Run { prompt } => {
            let actual_prompt = if prompt == "-" {
                let mut input = String::new();
                io::stdin().read_to_string(&mut input)?;
                input
            } else { prompt.clone() };
            agent.ask(&actual_prompt).await.map(|answer| println!("{answer}"))
        }
        Commands::Chat { .. } => chat(&mut agent, &config).await,
        Commands::Sdk { .. } => sdk(&mut agent, &config).await,
        _ => unreachable!(),
    };
    let end_result = agent.finish().await;
    result?;
    end_result

}

async fn chat(agent: &mut Agent, config: &Config) -> Result<()> {
    println!("Atlas Agent. /help lists commands; /exit quits.");
    loop {
        print!("atlas> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 { break; }
        let input = line.trim();
        if input.is_empty() { continue; }
        match input {
            "/exit" | "/quit" => break,
            "/help" => {
                println!("/model PROVIDER [MODEL]  switch provider or model");
                println!("/session                 show resumable session id");
                println!("/usage                   show provider-reported token totals");
                println!("/tasks                   list background shell tasks");
                println!("/clear                   erase current conversation history");
                println!("/compact                 summarize and compact conversation history");
                println!("/exit                    quit");
                continue;
            }
            "/session" => {
                println!("{}", agent.session_id());
                continue;
            }
            "/tasks" => {
                println!("{}",agent.background_tasks()?);
                continue;
            }
            "/usage" => {
                let usage = agent.usage();
                println!("Input: {}  Output: {}  Total: {} tokens",
                    usage.input_tokens, usage.output_tokens, usage.total_tokens);
                continue;
            }
            "/clear" => {
                agent.clear_history()?;
                println!("Conversation cleared.");
                continue;
            }
            "/compact" => {
                agent.compact_history().await?;
                println!("Session context compacted.");
                continue;
            }
            _ => {}
        }
        if let Some(rest) = input.strip_prefix("/model ") {
            let mut args = rest.split_whitespace();
            let Some(id) = args.next() else {
                eprintln!("Usage: /model PROVIDER [MODEL]");
                continue;
            };
            let override_model = args.next();
            let profile = config.select(Some(id), override_model)?;
            agent.switch_model(profile)?;
            println!("Model: {} / {}", agent.profile().id, agent.profile().model);
            continue;
        }
        match agent.ask(input).await {
            Ok(answer) => println!("{answer}"),
            Err(err) => eprintln!("Error: {err:#}"),
        }
    }
    Ok(())
}

async fn sdk(agent:&mut Agent,config:&Config)->Result<()>{
    print_json!(json!({"type":"system","subtype":"init",
        "session_id":agent.session_id(),"provider":agent.profile().id,
        "model":agent.profile().model,
        "usage_schema":runtime_contracts::empty_usage()}));
    let stdin=io::stdin();
    for input in stdin.lock().lines(){
        let line=input?;
        if line.trim().is_empty(){continue}
        let event:Value=match serde_json::from_str(&line){
            Ok(value)=>value,
            Err(error)=>{
                print_json!(json!({"type":"error","message":error.to_string()}));
                continue;
            }
        };
        let kind=event.get("type").and_then(Value::as_str).unwrap_or("user");
        match kind {
            "user"=>{
                let prompt=event.get("content").and_then(Value::as_str)
                    .or_else(||event.pointer("/message/content").and_then(Value::as_str));
                let Some(prompt)=prompt else{
                    print_json!(json!({"type":"error","message":"User event requires content"}));
                    continue;
                };
                match agent.ask(prompt).await{
                    Ok(answer)=>print_json!(json!({
                        "type":"result","session_id":agent.session_id(),
                        "content":answer,"usage":agent.usage()
                    })),
                    Err(error)=>print_json!(json!({
                        "type":"error","session_id":agent.session_id(),
                        "message":format!("{error:#}")
                    })),
                }
            },
            "control"=>{
                let action=event.get("action").and_then(Value::as_str).unwrap_or("");
                if action=="mailbox_send" {
                    let parsed=event.get("message").cloned()
                        .ok_or_else(||anyhow::anyhow!("mailbox_send requires message"))
                        .and_then(|v|serde_json::from_value::<in_memory_mailbox::Message>(v)
                            .map_err(anyhow::Error::from));
                    match parsed {
                        Ok(message)=>{
                            let revision=agent.send_transient(message);
                            print_json!(json!({"type":"control_response","action":action,
                                "ok":true,"revision":revision}));
                        }
                        Err(error)=>print_json!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if action=="mailbox_poll" {
                    let message=agent.poll_transient();
                    print_json!(json!({"type":"control_response","action":action,
                        "ok":true,"message":message,"revision":agent.transient_revision()}));
                    continue;
                }
                if action=="tasks" {
                    match agent.background_tasks() {
                        Ok(tasks)=>print_json!(json!({
                            "type":"control_response","action":action,
                            "ok":true,"result":serde_json::from_str::<Value>(&tasks)?
                        })),
                        Err(error)=>print_json!(json!({
                            "type":"control_response","action":action,
                            "ok":false,"error":format!("{error:#}")
                        })),
                    }
                    continue;
                }
                let result=match action {
                    "clear"=>agent.clear_history(),
                    "compact"=>agent.compact_history().await,
                    "model"=>{
                        let provider=event.get("provider").and_then(Value::as_str);
                        let model=event.get("model").and_then(Value::as_str);
                        config.select(provider,model).and_then(|p|agent.switch_model(p))
                    },
                    _=>Err(anyhow::anyhow!("Unknown control action: {action}")),
                };
                match result{
                    Ok(())=>print_json!(json!({"type":"control_response",
                        "action":action,"ok":true})),
                    Err(error)=>print_json!(json!({"type":"control_response",
                        "action":action,"ok":false,"error":format!("{error:#}")})),
                };
            },
            other=>print_json!(json!({"type":"error",
                "message":format!("Unsupported SDK event {other}")})),
        }
    }
    Ok(())
}
