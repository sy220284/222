mod agent;
mod cost;
mod tool_metrics;
mod collections;
mod message_queue;
mod query_guard;
mod sdk_queue;
mod queued_prompt_batch;
mod shell_quote;
mod skill_arguments;
mod semantic_inputs;
mod xml_escape;
mod json_read;
mod slash_command;
mod mcp_prompt;
mod multimodal;
mod hooks;
mod worktree;
mod mcp;
mod mcp_http;
mod config;
mod protocol;
mod provider;
mod session;
mod session_control;
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
mod gemini_sse;
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
mod control_message_compat;
mod permission_rules;
mod permission_mode;
mod reactive_store;
mod cleanup_registry;
mod workspace_dirs;

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

    /// Additional user-authorized directories, applicable to run/chat/SDK
    #[arg(long = "add-dir", global = true)]
    additional_directory: Vec<PathBuf>,

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
    /// List saved conversations in the current workspace
    Sessions { query:Option<String> },
    /// Give an existing conversation a persisted custom title
    Rename { session_id:String, title:String },
    /// Export a saved conversation, including tool activity
    Export { session_id:String, filename:Option<String> },
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
    if matches!(&cli.command,Commands::Sessions{..}|Commands::Rename{..}|Commands::Export{..}) {
        let root = canonical_workspace(&cli.workspace)?;
    match &cli.command{
        Commands::Sessions{query}=>{
            for item in session_control::search(&root,query.as_deref().unwrap_or(""))?{
                println!("{}  {}  ({} messages)",item.id,item.title,item.messages);
            }
            return Ok(())
        }
        Commands::Rename{session_id,title}=>{
            println!("{}",session_control::rename_saved(&root,session_id,title)?);
            return Ok(())
        }
        Commands::Export{session_id,filename}=>{
            let path=session_control::export_saved(&root,session_id,filename.as_deref())?;
            println!("Conversation exported to: {}",path.display());
            return Ok(())
        }
        _=>{}
    }
    }
    let mut config = Config::load()?;
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
        Commands::Chat { resume: Some(id) } => session_control::resolve(&root,id)?,
        Commands::Sdk { resume: Some(id) } => Session::load(&root, id)?,
        _ => Session::fresh(),
    };
    let mut agent = Agent::new(
        profile, root, cli.approval.into(), interactive, cli.max_steps, session
    )?;
    agent.attach_configured_directories(&config.additional_directories)?;
    for directory in &cli.additional_directory {
        let raw=directory.to_string_lossy();
        match agent.validate_workspace_directory(raw.as_ref())? {
            workspace_dirs::Validation::Success(path) => agent.add_workspace_directory(path),
            workspace_dirs::Validation::AlreadyAccessible{..} => {},
            _ => bail!("Invalid --add-dir directory: {}",directory.display()),
        }
    }
    agent.set_enabled_plugins(config.enabled_plugins.clone());
    agent.set_model_prices(&config.model_prices);
    agent.set_permission_rules(&config.permission_rules)?;
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
        Commands::Chat { .. } => chat(&mut agent, &mut config).await,
        Commands::Sdk { .. } => sdk(&mut agent, &config).await,
        _ => unreachable!(),
    };
    let end_result = agent.finish().await;
    result?;
    end_result

}

async fn chat(agent: &mut Agent, config: &mut Config) -> Result<()> {
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
                println!("/sessions [query]        search saved conversation titles");
                println!("/resume TERM             find session, show restart command");
                println!("/rename [NAME]           rename conversation or generate a name using active model");
                println!("/export [filename]       save full conversation to a local text file");
                println!("Resume: atlas-agent chat --resume SESSION_ID");
                println!("/usage                   show provider-reported token totals");
                println!("/cost                    show model costs and unknown pricing");
                println!("/tasks                   list background shell tasks");
                println!("/mode                    cycle permission mode (default/accept edits/plan)");
                println!("/add-dir PATH [--remember] add an authorized working directory");
                println!("/dirs                    show authorized working directories");
                println!("/clear                   erase current conversation history");
                println!("/compact                 summarize and compact conversation history");
                println!("/mcp-prompts             list available MCP prompt commands");
                println!("/server:prompt (MCP) ARGS  execute an MCP prompt");
                println!("/exit                    quit");
                continue;
            }
            "/dirs" => { for dir in agent.working_directories() { println!("{}",dir.display()); } continue; }
            "/session" => {
                println!("{}", agent.session_id());
                continue;
            }
            "/sessions" => {
                for item in agent.list_sessions()?{
                    println!("{}  {}  ({} messages)",item.id,item.title,item.messages);
                }
                continue;
            }
            "/mode" => {
                println!("Permission mode: {}",agent.cycle_permission_mode()?);
                continue;
            }
            "/tasks" => {
                println!("{}",agent.background_tasks()?);
                continue;
            }
            "/mcp-prompts" => {
                let catalog=agent.list_mcp_prompt_commands().await?;
                let cmds=catalog.as_array().expect("MCP commands are an array");
                if cmds.is_empty() {println!("No MCP prompt commands are available.");}
                for cmd in cmds{
                    println!("/{} — {}",cmd["userFacingName"].as_str().unwrap_or(""),
                        cmd["description"].as_str().unwrap_or(""));
                }
                continue;
            }
            "/cost" => {
                println!("{}", agent.cost_summary());
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
        if input.starts_with("/sessions ") {
            for item in agent.search_sessions(input.trim_start_matches("/sessions").trim())? {
                println!("{}  {}  ({} messages)",item.id,item.title,item.messages);
            }
            continue;
        }
        if input == "/resume" || input.starts_with("/resume ") {
            let query=input.trim_start_matches("/resume").trim();
            match session_control::resolve(
                agent.workspace_root(),query
            ){
                Ok(session)=>println!("To resume: atlas-agent --workspace . chat --resume {}",
                    session.id),
                Err(err)=>eprintln!("{err:#}"),
            }
            continue;
        }
        if input == "/rename" || input.starts_with("/rename ") {
            match agent.rename_session_or_generate(input.trim_start_matches("/rename").trim()).await{
                Ok(result)=>println!("{result}"),
                Err(err)=>eprintln!("{err:#}"),
            }
            continue;
        }
        if input == "/export" || input.starts_with("/export ") {
            let value=input.trim_start_matches("/export").trim();
            match agent.export_session(if value.is_empty(){None}else{Some(value)}){
                Ok(path)=>println!("Conversation exported to: {}",path.display()),
                Err(err)=>eprintln!("Failed to export conversation: {err:#}"),
            }
            continue;
        }
        if input == "/add-dir" || input.starts_with("/add-dir ") {
            let mut entered=String::new();
            let mut raw = input.trim_start_matches("/add-dir").trim();
            if raw.is_empty() {
                print!("Directory path: ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut entered)?;
                raw=entered.trim();
            }
            let remember = raw.ends_with(" --remember");
            if remember { raw = raw.trim_end_matches(" --remember").trim_end(); }
            match agent.validate_workspace_directory(raw)? {
                workspace_dirs::Validation::EmptyPath => eprintln!("Usage: /add-dir PATH [--remember]"),
                workspace_dirs::Validation::PathNotFound(p) => eprintln!("Path {} was not found",p.display()),
                workspace_dirs::Validation::NotADirectory(p) => eprintln!("{} is not a directory. Try {}",p.display(),p.parent().unwrap_or(&p).display()),
                workspace_dirs::Validation::AlreadyAccessible { path, existing_root } => println!("{} is already accessible via {}",path.display(),existing_root.display()),
                workspace_dirs::Validation::Success(path) => {
                    print!("Allow {}? [y] this session / [r] remember / [N] cancel: ",path.display());
                    io::stdout().flush()?;
                    let mut response=String::new();
                    io::stdin().read_line(&mut response)?;
                    let decision=response.trim().to_ascii_lowercase();
                    if matches!(decision.as_str(),"y"|"yes"|"r"|"remember") {
                        agent.add_workspace_directory(path.clone());
                        if remember || matches!(decision.as_str(),"r"|"remember") {
                            if !config.additional_directories.iter().any(|d| d == path.to_string_lossy().as_ref()) { config.additional_directories.push(path.to_string_lossy().to_string()); }
                            match config.save() {
                                Ok(()) => println!("Added {} and saved to local settings",path.display()),
                                Err(err) => eprintln!("Added {} for this session; settings save failed: {err:#}",path.display()),
                            }
                        } else { println!("Added {} for this session",path.display()); }
                    } else { println!("Did not add {}",path.display()); }
                }
            }
            continue;
        }
        if let Some(parsed) = slash_command::parse_slash_command(input)
            .filter(|parsed|parsed.command_name=="model" && !parsed.is_mcp) {
            let mut args = parsed.args.split_whitespace();
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
    let mut queue=message_queue::Queue::default();
    let mut query_guard=query_guard::QueryGuard::default();
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
        let event=control_message_compat::normalize_control_message_keys(event);
        let kind=event.get("type").and_then(Value::as_str).unwrap_or("user");
        match kind {
            "user"=>{
                let prompt_value=event.get("content")
                    .or_else(||event.pointer("/message/content"));
                let Some(prompt_value)=prompt_value else{
                    print_json!(json!({"type":"error","message":"User event requires content"}));
                    continue;
                };
                if event.get("queued").and_then(Value::as_bool)==Some(true){
                    let mut command:message_queue::Command=match serde_json::from_value(json!({
                        "value":prompt_value,"mode":"prompt","priority":event.get("priority")
                            .cloned().unwrap_or(json!("next")),
                        "skipSlashCommands":event.get("skipSlashCommands").cloned()
                            .unwrap_or(json!(false)),
                        "isMeta":event.get("isMeta").cloned().unwrap_or(json!(false)),
                        "workload":event.get("workload").cloned().unwrap_or(Value::Null),
                        "uuid":event.get("uuid").cloned().unwrap_or(Value::Null),
                        "origin":event.get("origin").cloned().unwrap_or(Value::Null),
                        "pastedContents":event.get("pastedContents").cloned().unwrap_or(Value::Null),
                        "preExpansionValue":event.get("preExpansionValue").cloned().unwrap_or(Value::Null),
                        "bridgeOrigin":event.get("bridgeOrigin").cloned().unwrap_or(json!(false))
                    })) {
                        Ok(command)=>command,
                        Err(error)=>{
                            print_json!(json!({"type":"error",
                                "message":format!("Invalid queued user command: {error}")}));
                            continue;
                        }
                    };
                    command.agent_id=event.get("agentId").and_then(Value::as_str).map(str::to_owned);
                    let identity=queue.enqueue(command,false);
                    print_json!(json!({"type":"queued","identity":identity,
                        "revision":queue.revision(),"length":queue.len()}));
                    continue;
                }
                // Preserve and validate SDK text/image blocks before routing
                // to a provider. Never silently flatten away image payloads.
                let user=match multimodal::parse(prompt_value){
                    Ok(user)=>user,
                    Err(error)=>{
                        print_json!(json!({"type":"error",
                            "message":format!("Unsupported user content: {error:#}")}));
                        continue;
                    }
                };
                // QueryEngine normally emits this replay during ask().
                // The Rust SDK entrypoint owns the delivery acknowledgement.
                if event.get("replay_user_messages")
                    .or_else(||event.get("replayUserMessages"))
                    .and_then(Value::as_bool)==Some(true) {
                    if let Some(uuid)=event.get("uuid").and_then(Value::as_str) {
                        sdk_queue::emit_user_replay(prompt_value,uuid,&agent.session_id());
                    }
                }
                match agent.ask_with_blocks(&user.preview,user.blocks).await{
                    Ok(answer)=>print_json!(json!({
                        "type":"result","session_id":agent.session_id(),
                        "content":answer,"usage":agent.usage(),"cost":agent.cost_ledger()
                    })),
                    Err(error)=>print_json!(json!({
                        "type":"error","session_id":agent.session_id(),
                        "message":format!("{error:#}")
                    })),
                }
            },
            "control_request" if event.get("request").is_some()=>{
                print_json!(sdk_nested_control(agent,config,&event,&mut queue).await);
            },
            "control" | "control_request"=>{
                macro_rules! send_control {
                    ($reply:expr) => {{
                        let mut response:Value=$reply;
                        if let Some(id)=event.get("request_id"){
                            response["request_id"]=id.clone();
                        }
                        print_json!(response);
                    }};
                }
                let action=event.get("action").and_then(Value::as_str).unwrap_or("");
                if action.starts_with("queue_"){
                    let outcome=sdk_queue::control(action,&event,&mut queue,
                        &mut query_guard,agent).await;
                    match outcome {
                        Ok(result)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":result})),
                        Err(error)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if action=="mcp_prompt_commands" {
                    let outcome=agent.list_mcp_prompt_commands().await;
                    match outcome{
                        Ok(value)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":value})),
                        Err(error)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if matches!(action,"mcp_resources"|"mcp_prompts"|
                    "mcp_resource_templates"|"mcp_read_resource"|"mcp_get_prompt"|
                    "mcp_resource_subscribe"|"mcp_resource_unsubscribe"|
                    "mcp_resource_updates") {
                    let result=mcp_control_request(agent,action,&event).await;
                    match result{
                        Ok(value)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":value})),
                        Err(error)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    };
                    continue;
                }
                if action=="mailbox_send" {
                    let parsed=event.get("message").cloned()
                        .ok_or_else(||anyhow::anyhow!("mailbox_send requires message"))
                        .and_then(|v|serde_json::from_value::<in_memory_mailbox::Message>(v)
                            .map_err(anyhow::Error::from));
                    match parsed {
                        Ok(message)=>{
                            let revision=agent.send_transient(message);
                            send_control!(json!({"type":"control_response","action":action,
                                "ok":true,"revision":revision}));
                        }
                        Err(error)=>send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if action=="mailbox_poll" {
                    let message=agent.poll_transient();
                    send_control!(json!({"type":"control_response","action":action,
                        "ok":true,"message":message,"revision":agent.transient_revision()}));
                    continue;
                }
                if action=="cost" {
                    send_control!(json!({"type":"control_response","action":action,
                        "ok":true,"cost":agent.cost_ledger()}));
                    continue;
                }
                if action=="permission_mode" {
                    send_control!(json!({"type":"control_response","action":action,
                        "ok":true,"mode":agent.current_permission_mode()}));
                    continue;
                }
                if action=="tasks" {
                    match agent.background_tasks() {
                        Ok(tasks)=>send_control!(json!({
                            "type":"control_response","action":action,
                            "ok":true,"result":serde_json::from_str::<Value>(&tasks)?
                        })),
                        Err(error)=>send_control!(json!({
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
                    Ok(())=>send_control!(json!({"type":"control_response",
                        "action":action,"ok":true})),
                    Err(error)=>send_control!(json!({"type":"control_response",
                        "action":action,"ok":false,"error":format!("{error:#}")})),
                };
            },
            other=>print_json!(json!({"type":"error",
                "message":format!("Unsupported SDK event {other}")})),
        }
    }
    Ok(())
}


/// Standard SDK request envelope; the legacy flat local command protocol
/// remains available through "control" messages.
async fn sdk_nested_control(agent:&mut Agent,config:&Config,event:&Value,
    queue:&mut message_queue::Queue)->Value{
    let id=event.get("request_id").and_then(Value::as_str).unwrap_or("");
    let outcome:Result<Value>=async {
        if id.is_empty(){bail!("control_request requires request_id")}
        let request=event.get("request").and_then(Value::as_object)
            .ok_or_else(||anyhow::anyhow!("control_request requires request object"))?;
        let subtype=request.get("subtype").and_then(Value::as_str)
            .ok_or_else(||anyhow::anyhow!("control request requires subtype"))?;
        match subtype{
            "initialize"=>{
                // The Rust session is already attached to locally configured
                // hooks/MCP before stdin is read. Never silently claim to have
                // installed SDK callback hooks or SDK-owned MCP transports.
                for key in ["hooks","sdkMcpServers","jsonSchema","systemPrompt",
                    "appendSystemPrompt","agents","promptSuggestions",
                    "agentProgressSummaries"] {
                    if let Some(value)=request.get(key) {
                        if !value.is_null() && value!=&json!({}) &&
                            value!=&json!([]) && value!=&json!(false) {
                            bail!("initialize {key} is not supported by this local SDK")
                        }
                    }
                }
                let models=config.providers.iter().map(|profile|json!({
                    "value":profile.model,"displayName":format!("{} / {}",
                        profile.id,profile.model),
                    "description":format!("{} provider",profile.kind)
                })).collect::<Vec<_>>();
                Ok(json!({"commands":[],"agents":[],"output_style":"default",
                    "available_output_styles":["default"],"models":models,
                    "account":{},"pid":std::process::id()}))
            },
            "mcp_status"=>{
                Ok(json!({"mcpServers":agent.mcp_statuses()?}))
            },
            "mcp_prompt_commands"=>agent.list_mcp_prompt_commands().await,
            "mcp_resources"|"mcp_prompts"|"mcp_resource_templates"|
            "mcp_read_resource"|"mcp_get_prompt"|
            "mcp_resource_subscribe"|"mcp_resource_unsubscribe"|
            "mcp_resource_updates"=>{
                mcp_control_request(agent,subtype,&Value::Object(request.clone())).await
            },
            "cancel_async_message"=>{
                let uuid=request.get("message_uuid").and_then(Value::as_str)
                    .filter(|id|!id.is_empty())
                    .ok_or_else(||anyhow::anyhow!(
                        "cancel_async_message requires non-empty message_uuid"))?;
                // Original 2.1.88 uses dequeueAllMatching, not priority dequeue.
                // Already-dispatched messages cannot be retroactively cancelled.
                let removed=queue.dequeue_all_matching(|item|
                    item.uuid.as_deref()==Some(uuid));
                Ok(json!({"cancelled":!removed.is_empty()}))
            },
            "set_model"=>{
                if let Some(name)=request.get("model"){
                    let model=name.as_str().ok_or_else(||
                        anyhow::anyhow!("set_model model must be a string"))?;
                    let profile=config.select(Some(&agent.profile().id),Some(model))?;
                    agent.switch_model(profile)?;
                }
                Ok(json!({"model":agent.profile().model,"provider":agent.profile().id}))
            },
            "set_permission_mode"=>{
                let mode=request.get("mode").and_then(Value::as_str)
                    .ok_or_else(||anyhow::anyhow!("set_permission_mode requires mode"))?;
                agent.set_sdk_permission_mode(mode)?;
                Ok(json!({"mode":agent.current_permission_mode()}))
            },
            "stop_task"=>{
                let task_id=request.get("task_id").and_then(Value::as_str)
                    .ok_or_else(||anyhow::anyhow!("stop_task requires task_id"))?;
                let result=agent.stop_background_task(task_id).await?;
                Ok(json!({"task":serde_json::from_str::<Value>(&result)?}))
            },
            other=>bail!("Unsupported SDK control subtype: {other}"),
        }
    }.await;
    match outcome{
        Ok(response)=>json!({"type":"control_response","response":{
            "subtype":"success","request_id":id,"response":response
        }}),
        Err(error)=>json!({"type":"control_response","response":{
            "subtype":"error","request_id":id,"error":format!("{error:#}")
        }}),
    }
}


/// Explicit SDK MCP introspection; never invoke arbitrary server methods.
async fn mcp_control_request(agent:&mut Agent,action:&str,request:&Value)
    ->Result<Value>{
    match action {
        "mcp_resources"=>agent.mcp_catalog("resources").await,
        "mcp_prompts"=>agent.mcp_catalog("prompts").await,
        "mcp_resource_templates"=>agent.mcp_catalog("resource_templates").await,
        "mcp_resource_updates"=>Ok(agent.mcp_resource_updates()),
        "mcp_resource_subscribe"|"mcp_resource_unsubscribe"=>{
            let server=request.get("server").and_then(Value::as_str)
                .ok_or_else(||anyhow::anyhow!("MCP subscription requires server"))?;
            let uri=request.get("uri").and_then(Value::as_str)
                .ok_or_else(||anyhow::anyhow!("MCP subscription requires uri"))?;
            agent.mcp_subscribe_resource(server,uri,
                action=="mcp_resource_subscribe").await
        },
        "mcp_read_resource"=>{
            let server=request.get("server").and_then(Value::as_str)
                .ok_or_else(||anyhow::anyhow!("mcp_read_resource requires server"))?;
            let uri=request.get("uri").and_then(Value::as_str)
                .ok_or_else(||anyhow::anyhow!("mcp_read_resource requires uri"))?;
            agent.mcp_read_resource(server,uri).await
        }
        "mcp_get_prompt"=>{
            let server=request.get("server").and_then(Value::as_str)
                .ok_or_else(||anyhow::anyhow!("mcp_get_prompt requires server"))?;
            let name=request.get("name").and_then(Value::as_str)
                .ok_or_else(||anyhow::anyhow!("mcp_get_prompt requires name"))?;
            let empty=serde_json::Map::new();
            let arguments=match request.get("arguments"){
                None|Some(Value::Null)=>&empty,
                Some(Value::Object(args))=>args,
                _=>bail!("mcp_get_prompt arguments must be an object"),
            };
            agent.mcp_get_prompt(server,name,arguments).await
        }
        _=>bail!("Unknown MCP SDK operation"),
    }
}
