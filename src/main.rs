mod agent;
mod hooks;
mod worktree;
mod mcp;
mod config;
mod protocol;
mod provider;
mod session;
mod task;
mod tools;
mod notebook;
mod search;
mod lsp;

use agent::Agent;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use config::{canonical_workspace, Config};
use session::Session;
use std::{
    io::{self, Read, Write},
    path::PathBuf,
};
use tools::Approval;

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
    let profile = config.select(cli.provider.as_deref(), cli.model.as_deref())?;
    let interactive = matches!(&cli.command, Commands::Chat { .. });
    let session = match &cli.command {
        Commands::Chat { resume: Some(id) } => Session::load(&root, id)?,
        _ => Session::fresh(),
    };
    let mut agent = Agent::new(
        profile, root, cli.approval.into(), interactive, cli.max_steps, session
    )?;
    agent.attach_mcp(&config.mcp_servers).await?;
    agent.attach_lsp(&config.lsp_servers).await?;
    agent.attach_hooks(&config.hooks).await?;
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
                println!("/clear                   erase current conversation history");
                println!("/compact                 summarize and compact conversation history");
                println!("/exit                    quit");
                continue;
            }
            "/session" => {
                println!("{}", agent.session_id());
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
