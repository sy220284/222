mod advisor_command;
mod agent;
mod agent_definition;
mod array_utils;
mod ask_user_question;
mod background;
mod bash_permission_suggestions;
mod browser_launcher;
mod chatgpt_plan;
mod cleanup_registry;
mod collections;
mod command_lifecycle;
mod command_manifest;
mod compact_command_source;
mod compat_helpers;
mod config;
mod context_usage;
mod control_message_compat;
mod conversation_context;
mod copy_response;
mod cost;
mod custom_multi_select_source;
mod custom_select_exports;
mod custom_select_input_source;
mod custom_select_source;
mod deep_link;
mod deep_link_banner;
mod deep_link_protocol;
mod deep_link_register;
mod detect_repository;
mod diagnostics;
mod diff_view;
mod display_format;
mod env_runtime;
mod env_validation;
mod event_signal;
mod exec_file;
mod exec_sync_compat;
mod files_context;
mod fs_operations;
mod gemini_content;
mod gemini_sse;
mod generated_ports;
mod git_config_parser;
mod git_filesystem;
mod git_ignore;
mod git_workflows;

mod github_repo_mapping;
mod history;
mod hooks;
mod in_memory_mailbox;
mod ink_constants;
mod json_read;
mod json_runtime;
mod keybindings;
mod local_advanced_keywords;
mod local_advanced_workflows;
mod local_attachments;
mod local_triggers;
mod lsp;
mod marketplace_registry;
mod marketplace_source;
mod mcp;
mod mcp_cli;
mod mcp_http;
mod mcp_legacy_sse;
mod mcp_output_storage;
mod mcp_policy;
mod mcp_prompt;
mod memory;
mod memory_age;
mod memory_extraction;
mod message_queue;
mod model_catalog;
mod multimodal;
mod ndjson;
mod notebook;
mod oauth_callback_listener;
mod oauth_client;
mod oauth_crypto;
mod oauth_profile;
mod oauth_service;
mod path_support;
mod permission_mode;
mod permission_rules;
mod permission_settings;
mod plain_text_storage;
mod platform_source_utilities;
mod plugin_commands;
mod plugin_error_messages_source;
mod plugin_manager;
mod plugin_pagination_source;
mod plugin_trust_warning_source;
mod plugins;
mod portable_async_runtime;
mod portable_configuration;
mod portable_ink_core;
mod portable_missing_utilities;
mod portable_platform;
mod portable_render_semantics;
mod portable_settings_runtime;
mod portable_source_utils;
mod portable_ui_runtime;
mod powershell_permission_matching;
mod project_init;
mod project_onboarding;
mod prompt_history;
mod prompt_runtime;
mod protocol;
mod provider;
mod query_config;
mod query_guard;
mod query_token_budget;
mod queue_journal;
mod queued_prompt_batch;
mod reactive_store;
mod release_notes;
mod runtime_contracts;
mod sdk_queue;
mod search;
mod secure_fallback;
mod semantic_inputs;
mod semantic_session_search;
mod session;
mod session_color;
mod session_control;
mod shell_quote;
mod shell_runtime;
mod side_question;
mod skill_arguments;
mod slash_command;
mod sleep_helpers;
mod source_semantics;
mod source_semantics_osc;
mod source_semantics_types;
mod sse;
mod stats;
mod statusline_command_source;
mod streaming_tool_executor;
mod subprocess_env;
mod task;
mod task_events;
mod task_graph;
mod task_guards;
mod task_output_format;
mod team_mailbox;
mod team_registry;
mod terminal_input;
mod terminal_launcher;
#[cfg(test)]
mod test_http;
mod tls_ca;
mod tool_errors;
mod tool_metrics;
mod tool_names;
mod tool_orchestration;
mod tool_search;
mod tools;
mod transcript_search;
mod unicode_sanitization;
mod version;
mod vim;
mod vim_mode_source;
mod webfetch;
mod websearch;
mod workspace_dirs;
mod worktree;
mod xml_escape;

macro_rules! print_json {
    ($value:expr) => {{
        println!("{}", crate::ndjson::stringify_value(&$value));
    }};
}

use agent::Agent;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use config::{canonical_workspace, Config};
use plugins::PluginSet;
use serde_json::{json, Value};
use session::Session;
use std::io::BufRead;
use std::{
    io::{self, Read, Write},
    path::PathBuf,
};
use tools::Approval;

#[derive(Parser, Debug)]
#[command(
    name = "atlas-agent",
    version,
    about = "Standalone multi-model Rust coding agent"
)]
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
    /// Display the version of this running binary
    Version,
    /// Create a secure OAuth PKCE verifier/challenge and state
    OauthPkce,
    /// Receive a browser authorization-code redirect on localhost
    OauthListen {
        #[arg(long)]
        state: String,
        #[arg(long)]
        success_url: String,
        #[arg(long, default_value = "/callback")]
        callback_path: String,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Run OAuth 2.0 authorization-code flow for any compatible provider
    OauthLogin {
        #[arg(long)]
        authorization_url: String,
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        client_id: String,
        #[arg(long = "scope", required = true)]
        scopes: Vec<String>,
        #[arg(long)]
        manual_redirect_url: String,
        #[arg(long)]
        credentials_file: PathBuf,
        #[arg(long)]
        success_url: Option<String>,
        #[arg(long)]
        error_url: Option<String>,
        #[arg(long)]
        manual: bool,
        #[arg(long)]
        login_hint: Option<String>,
    },
    /// Refresh an existing provider OAuth token and atomically save it
    OauthRefresh {
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        credentials_file: PathBuf,
    },
    /// Create a user-owned JSON config file with example providers
    Init,
    /// Inspect local installation, settings, Git and MCP availability offline
    Doctor,
    /// Manage MCP server configuration without connecting to the server
    Mcp {
        #[command(subcommand)]
        action: mcp_cli::McpCommand,
    },
    /// Set up project instructions, skills and hooks through guided model tools
    ProjectInit,
    /// Validate or save a user-defined multi-model agent from JSON
    AgentDefinition {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        user: bool,
        #[arg(long)]
        replace: bool,
        #[arg(long)]
        validate_only: bool,
    },
    /// Edit or validate user keyboard shortcuts (without model access)
    Keybindings {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        action: Option<String>,
        #[arg(long, default_value = "Chat")]
        context: String,
        #[arg(long)]
        no_edit: bool,
    },
    /// Report actual local session and model usage statistics
    Stats,
    /// Read a value from an existing Git configuration without invoking Git
    GitConfig {
        section: String,
        key: String,
        #[arg(long)]
        subsection: Option<String>,
        #[arg(long)]
        git_dir: Option<PathBuf>,
    },
    /// Inspect Git HEAD, branch and worktree state without contacting a remote
    GitState,
    /// Check Git ignore status, or explicitly append a global ignore rule
    GitIgnore {
        pattern: String,
        #[arg(long)]
        add_global: bool,
    },
    /// Resolve a link to the most recently used matching local checkout
    ResolveDeepLink {
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Show external-link origin, checkout path and Git fetch freshness
    DeepLinkBanner {
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        prefill_length: Option<usize>,
    },
    /// Parse an OS-invoked deep link and open the current binary in a terminal
    HandleUri { uri: String },
    /// Register atlas-cli:// links on this device, or inspect current registration
    RegisterProtocol {
        #[arg(long)]
        check: bool,
    },
    /// Persist the macOS terminal used when a browser opens an external link
    TerminalPreference {
        /// iTerm, Ghostty, kitty, Alacritty, WezTerm, Terminal, or auto
        terminal: Option<String>,
    },
    /// Parse a provider-neutral deep link without opening a terminal
    ParseDeepLink { uri: String },
    /// Encode an atlas-cli://open link for a reviewable prompt/workspace
    BuildDeepLink {
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Inspect newly implemented Rust source exports without connecting a model
    InspectSource {
        source: String,
        #[arg(long)]
        export: Option<String>,
        #[arg(long)]
        args: Option<String>,
    },
    /// Render ANSI-formatted text as SVG using the native Rust terminal renderer
    RenderAnsiSvg {
        /// Text containing ANSI escape sequences (stdin if omitted)
        #[arg(long)]
        text: Option<String>,
        /// SVG rendering options JSON (fontSize, paddingX, etc.)
        #[arg(long)]
        options: Option<String>,
    },
    /// Manage installed local plugins and parse the original plugin commands
    Plugin {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show cached changelog, optionally refreshing from a configured source
    ReleaseNotes,
    /// Read, update or delete local credentials using primary/fallback stores
    Credential {
        action: String,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        fallback_file: Option<PathBuf>,
    },
    /// Open an HTTP(S) URL with the user's operating-system browser
    OpenBrowser { url: String },
    /// Open a local path with the default system handler
    OpenPath { path: String },
    /// Clear screen and supported terminal scrollback
    ClearScreen,
    /// Show configured providers and active default without connecting to them
    Models {
        #[command(subcommand)]
        action: Option<model_catalog::ModelAction>,
    },
    /// Manage official Sign in with ChatGPT and subscription usage.
    Chatgpt {
        #[command(subcommand)]
        action: chatgpt_plan::AccountAction,
    },
    /// Perform a local PR review through the selected model and GitHub CLI
    Review { pr_number: Option<u64> },
    /// Deep source-based review using the active API model and local tools
    UltraReview { pr_number: Option<u64> },
    /// Detailed read-only project plan using the active API model
    UltraPlan { objective: Vec<String> },
    /// Manage user-owned, explicitly invoked local task triggers
    Trigger {
        #[command(subcommand)]
        action: LocalTriggerAction,
    },
    /// Review staged/unstaged changes and create a new Git commit
    Commit { instructions: Option<String> },
    /// Commit, push and create/update a Chinese pull request
    CommitPushPr { instructions: Option<String> },
    /// Execute a single prompt; pass '-' to read the prompt from stdin
    Run { prompt: String },
    /// Start an interactive conversation (optionally resume an earlier UUID)
    Chat {
        #[arg(long)]
        resume: Option<String>,
        #[arg(long)]
        prefill: Option<String>,
        #[arg(long)]
        deep_link_origin: bool,
        #[arg(long)]
        deep_link_repo: Option<String>,
        #[arg(long)]
        deep_link_last_fetch: Option<u64>,
    },
    /// List saved conversations in the current workspace
    Sessions { query: Option<String> },
    /// Find relevant conversation sessions using the configured model
    SemanticSessions { query: String },
    /// Search visible content of a saved conversation, without a model
    TranscriptSearch { session_id: String, query: String },
    /// Give an existing conversation a persisted custom title
    Rename { session_id: String, title: String },
    /// Export a saved conversation, including tool activity
    Export {
        session_id: String,
        filename: Option<String>,
    },
    /// JSON-lines SDK mode for external programs (no proprietary transport)
    Sdk {
        #[arg(long)]
        resume: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum LocalTriggerAction {
    /// Save or replace a reusable local task
    Set { name: String, prompt: Vec<String> },
    /// Create only if the task does not yet exist
    Create { name: String, prompt: Vec<String> },
    /// Update the prompt of an existing saved task
    Update { name: String, prompt: Vec<String> },
    /// Read the saved prompt of a task
    Get { name: String },
    /// Execute an existing saved task through the active Agent and model
    Run { name: String },
    /// Show saved local trigger names
    List,
    /// Delete a local trigger
    Delete { name: String },
}

fn main() -> Result<()> {
    // The single CLI dispatcher carries numerous async command branches.
    // Windows reserves a small stack for its initial process thread, so
    // instantiate and poll this dispatcher on a dedicated, sized runtime
    // thread. Keep all Tokio execution and process cleanup on that thread.
    let thread =
        std::thread::Builder::new()
            .name("atlas-cli-runtime".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| -> Result<()> {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async {
                    let result = run_cli().await;
                    let cleanup = cleanup_registry::run_cleanup_functions().await;
                    match (result, cleanup) {
                        (Err(error), Err(cleanup_error)) => Err(error
                            .context(format!("Process cleanup also failed: {cleanup_error:#}"))),
                        (Err(error), _) => Err(error),
                        (Ok(()), result) => result,
                    }
                })
            })?;
    match thread.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// Preserve arbitrary SDK message identifiers without losing the UUID used
/// for internal file snapshots. External IDs remain unchanged in SDK replies;
/// non-UUID IDs receive a stable session-scoped, RFC 9562 version-8 UUID.
fn sdk_message_id(external: &str, session_id: &str) -> uuid::Uuid {
    if let Ok(uuid) = uuid::Uuid::parse_str(external) {
        return uuid;
    }
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"atlas-sdk-message-id-v1:");
    digest.update(session_id.as_bytes());
    digest.update(b":");
    digest.update(external.as_bytes());
    let hash = digest.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

// src/commands/output-style/output-style.tsx::call. Keep the legacy
// command's system notice in one place; config changes apply next session.
const LEGACY_OUTPUT_STYLE_NOTICE: &str = "/output-style has been deprecated. Use /config to change your output style, or set it in your settings file. Changes take effect on the next session.";

async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    // Doctor must diagnose malformed user configuration without failing startup.
    if !matches!(cli.command, Commands::Doctor) {
        tls_ca::apply_user_startup_config()?;
    }
    match &cli.command {
        Commands::Chatgpt { action } => {
            chatgpt_plan::run(action).await?;
            return Ok(());
        }
        Commands::ReleaseNotes => {
            let home = keybindings::config_home()?;
            let url = std::env::var("ATLAS_RELEASE_NOTES_URL").ok();
            println!("{}", release_notes::show(&home, url.as_deref()).await?);
            return Ok(());
        }
        Commands::Credential {
            action,
            file,
            fallback_file,
        } => {
            use secure_fallback::SecureStorage;
            let selected = match file {
                Some(path) => path.clone(),
                None => {
                    plain_text_storage::PlainTextStorage::default_location()
                        .ok_or_else(|| anyhow::anyhow!("Cannot determine credential directory"))?
                        .path
                }
            };
            let primary: std::sync::Arc<dyn SecureStorage> =
                std::sync::Arc::new(plain_text_storage::PlainTextStorage::new(selected));
            let storage: std::sync::Arc<dyn SecureStorage> = if let Some(path) = fallback_file {
                std::sync::Arc::new(secure_fallback::FallbackStorage::new(
                    primary,
                    std::sync::Arc::new(plain_text_storage::PlainTextStorage::new(path.clone())),
                ))
            } else {
                primary
            };
            match action.as_str() {
                "read" => println!("{}", serde_json::to_string_pretty(&storage.read())?),
                "write" => {
                    let mut text = String::new();
                    io::stdin().read_to_string(&mut text)?;
                    let value: Value = serde_json::from_str(&text)?;
                    let result = storage.update(value);
                    if !result.success {
                        bail!("Unable to persist credentials");
                    }
                    if let Some(warning) = result.warning {
                        eprintln!("{warning}");
                    }
                    println!("Credentials updated");
                }
                "delete" => {
                    if !storage.delete() {
                        bail!("Unable to delete stored credentials");
                    }
                    println!("Credentials deleted");
                }
                "status" => println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "storage":storage.name(),
                        "available":storage.read().is_some()
                    }))?
                ),
                _ => bail!("Credential action must be read, write, delete, or status"),
            }
            return Ok(());
        }
        Commands::Mcp { action } => {
            let root = canonical_workspace(&cli.workspace)?;
            println!("{}", mcp_cli::run(&root, action).await?);
            return Ok(());
        }
        Commands::Plugin { args } => {
            let root = canonical_workspace(&cli.workspace)?;
            let command = plugin_commands::parse_plugin_args(Some(&args.join(" ")));
            let output = plugin_manager::execute(&root, &command)?;
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }
        Commands::TerminalPreference { terminal } => {
            let mut config = Config::load()?;
            if let Some(value) = terminal {
                let normalized = value.trim().trim_end_matches(".app").to_ascii_lowercase();
                let preferred = match normalized.as_str() {
                    "auto" => None,
                    "iterm" | "iterm2" => Some("iTerm"),
                    "ghostty" => Some("Ghostty"),
                    "kitty" => Some("kitty"),
                    "alacritty" => Some("Alacritty"),
                    "wezterm" => Some("WezTerm"),
                    "terminal" => Some("Terminal"),
                    _ => bail!(
                        "Terminal must be iTerm, Ghostty, kitty, Alacritty, WezTerm, Terminal or auto"
                    ),
                };
                config.deep_link_terminal = preferred.map(str::to_owned);
                config.save()?;
            }
            println!(
                "Deep-link terminal: {}",
                config.deep_link_terminal.as_deref().unwrap_or("auto")
            );
            return Ok(());
        }
        Commands::OpenBrowser { url } => {
            if !browser_launcher::open_browser(url).await {
                bail!("Unable to open URL in system browser");
            }
            return Ok(());
        }
        Commands::OpenPath { path } => {
            if !browser_launcher::open_path(path).await {
                bail!("Unable to open path in default OS handler");
            }
            return Ok(());
        }
        Commands::ClearScreen => {
            print!(
                "{}",
                platform_source_utilities::get_clear_terminal_sequence()
            );
            io::stdout().flush()?;
            return Ok(());
        }
        Commands::Version => {
            println!("{}", version::running_version());
            return Ok(());
        }
        Commands::OauthListen {
            state,
            success_url,
            callback_path,
            port,
        } => {
            let mut listener =
                oauth_callback_listener::AuthCodeListener::start(callback_path, *port).await?;
            println!(
                "OAuth callback listening on http://127.0.0.1:{}{}",
                listener.port(),
                callback_path
            );
            io::stdout().flush()?;
            let authorization = listener
                .wait_for_authorization(state, || async { Ok(()) })
                .await;
            match authorization {
                Ok(code) => {
                    listener.handle_success_redirect(success_url).await?;
                    listener.close().await?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "authorization_code":code
                        }))?
                    );
                }
                Err(error) => {
                    listener.close().await?;
                    return Err(error);
                }
            }
            return Ok(());
        }
        Commands::OauthLogin {
            authorization_url,
            token_url,
            client_id,
            scopes,
            manual_redirect_url,
            credentials_file,
            success_url,
            error_url,
            manual,
            login_hint,
        } => {
            let provider = oauth_client::OAuthProvider::new(
                authorization_url,
                token_url,
                client_id,
                scopes.clone(),
                manual_redirect_url,
            )?;
            let service = oauth_service::OAuthService::new()?;
            let options = oauth_client::AuthorizationOptions {
                login_hint: login_hint.clone(),
                ..Default::default()
            };
            if *manual {
                println!(
                    "Open the following authorization URL:\n{}",
                    service.manual_authorization_url(&provider, &options)
                );
                println!("Paste the authorization code:");
                io::stdout().flush()?;
                let mut code = String::new();
                io::stdin().read_line(&mut code)?;
                let tokens = service.finish_manual(&provider, code.trim()).await?;
                oauth_service::save_oauth_tokens(credentials_file, &tokens)?;
            } else {
                let success = success_url
                    .as_deref()
                    .unwrap_or("http://localhost/oauth/success");
                let error = error_url
                    .as_deref()
                    .unwrap_or("http://localhost/oauth/error");
                service
                    .run_automatic(
                        &provider,
                        &options,
                        success,
                        error,
                        credentials_file,
                        |links| async move {
                            println!("Open the automatic authorization URL:\n{}", links.automatic);
                            println!(
                                "Manual authorization URL (for other devices):\n{}",
                                links.manual
                            );
                            io::stdout().flush()?;
                            Ok(())
                        },
                    )
                    .await?;
            }
            println!("OAuth credentials saved: {}", credentials_file.display());
            return Ok(());
        }
        Commands::OauthRefresh {
            token_url,
            client_id,
            credentials_file,
        } => {
            let existing = oauth_service::load_oauth_tokens(credentials_file)?;
            let provider = oauth_client::OAuthProvider::new(
                token_url,
                token_url,
                client_id,
                existing.scopes.clone(),
                token_url,
            )?;
            let refreshed = provider
                .refresh_oauth_token(&existing.refresh_token, Some(&existing.scopes))
                .await?;
            oauth_service::save_oauth_tokens(credentials_file, &refreshed)?;
            println!(
                "OAuth credentials refreshed: {}",
                credentials_file.display()
            );
            return Ok(());
        }
        Commands::OauthPkce => {
            let request = oauth_crypto::PkceRequest::create()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "verifier":request.verifier,
                    "challenge":request.challenge,
                    "challenge_method":"S256",
                    "state":request.state
                }))?
            );
            return Ok(());
        }
        Commands::GitConfig {
            section,
            key,
            subsection,
            git_dir,
        } => {
            let root = canonical_workspace(&cli.workspace)?;
            let directory = git_dir.clone().unwrap_or_else(|| {
                git_filesystem::resolve_git_dir(&root).unwrap_or_else(|| root.join(".git"))
            });
            let result = git_config_parser::parse_git_config_value(
                &directory,
                section,
                subsection.as_deref(),
                key,
            );
            match result {
                Some(value) => println!("{value}"),
                None => println!("(not found)"),
            }
            return Ok(());
        }
        Commands::GitState => {
            let root = canonical_workspace(&cli.workspace)?;
            let state = git_filesystem::snapshot(&root);
            let remote = state
                .remote_url
                .as_deref()
                .and_then(detect_repository::parse_git_remote);
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "branch":state.branch,
                    "head":state.head,
                    "default_branch":state.default_branch,
                    "shallow":state.shallow,
                    "worktree_count":state.worktree_count,
                    "remote":remote.map(|repo| json!({
                        "host":repo.host,
                        "owner":repo.owner,
                        "name":repo.name
                    }))
                }))?
            );
            return Ok(());
        }
        Commands::GitIgnore {
            pattern,
            add_global,
        } => {
            let root = canonical_workspace(&cli.workspace)?;
            if *add_global {
                let changed = git_ignore::add_file_glob_rule_to_gitignore(pattern, &root)?;
                println!(
                    "{}",
                    if changed {
                        "Global ignore rule added"
                    } else {
                        "No change"
                    }
                );
            } else {
                println!("{}", git_ignore::is_path_gitignored(pattern, &root));
            }
            return Ok(());
        }
        Commands::ResolveDeepLink { cwd, repo } => {
            let config = Config::load()?;
            let resolved =
                github_repo_mapping::resolve_cwd(&config, cwd.as_deref(), repo.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "cwd":resolved.cwd,
                    "resolved_repo":resolved.resolved_repo
                }))?
            );
            return Ok(());
        }
        Commands::DeepLinkBanner {
            cwd,
            repo,
            prefill_length,
        } => {
            let root = canonical_workspace(cwd.as_deref().unwrap_or(&cli.workspace))?;
            // Only report the repository as resolved when it really is the
            // origin of the selected local checkout.
            let found = detect_repository::detect_current_repository(&root);
            let resolved = repo
                .as_deref()
                .filter(|slug| found.as_deref() == Some(*slug))
                .map(ToOwned::to_owned);
            let last_fetch = resolved
                .as_ref()
                .and_then(|_| deep_link_banner::read_last_fetch_time(&root));
            let info = deep_link_banner::DeepLinkBannerInfo {
                cwd: root,
                repo: resolved,
                prefill_length: *prefill_length,
                last_fetch,
            };
            println!("{}", deep_link_banner::build_deep_link_banner(&info));
            return Ok(());
        }
        Commands::RegisterProtocol { check } => {
            let executable = std::env::current_exe()?;
            if *check {
                println!(
                    "{}",
                    deep_link_register::is_protocol_handler_current(&executable)
                );
            } else {
                let changed = deep_link_register::ensure_protocol_handler_current(&executable)?;
                println!(
                    "{}",
                    if changed {
                        "Registered atlas-cli://"
                    } else {
                        "Already registered"
                    }
                );
            }
            return Ok(());
        }
        Commands::HandleUri { uri } => {
            let config = Config::load()?;
            deep_link_protocol::handle_deep_link_uri(uri, &config)?;
            return Ok(());
        }
        Commands::ParseDeepLink { uri } => {
            let action = deep_link::parse_deep_link(uri)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "query":action.query,"cwd":action.cwd,"repo":action.repo
                }))?
            );
            return Ok(());
        }
        Commands::BuildDeepLink { query, cwd, repo } => {
            let url = deep_link::build_deep_link(&deep_link::DeepLinkAction {
                query: query.clone(),
                cwd: cwd.clone(),
                repo: repo.clone(),
            })?;
            // The parser is the single validation and sanitization boundary.
            let action = deep_link::parse_deep_link(&url)?;
            println!("{}", deep_link::build_deep_link(&action)?);
            return Ok(());
        }
        _ => {}
    }

    if let Commands::AgentDefinition {
        file,
        user,
        replace,
        validate_only,
    } = &cli.command
    {
        let spec: agent_definition::AgentDefinition =
            serde_json::from_slice(&std::fs::read(file)?)?;
        let validation = agent_definition::validate_in_workspace(&spec, &cli.workspace, *user)?;
        for warning in &validation.warnings {
            eprintln!("warning: {warning}");
        }
        if !validation.valid() {
            bail!("{}", validation.errors.join("; "));
        }
        if *validate_only {
            println!("Agent definition valid: {}", spec.agent_type);
        } else {
            println!(
                "{}",
                agent_definition::save_agent(&spec, &cli.workspace, *user, *replace)?.display()
            );
        }
        return Ok(());
    }
    if let Commands::Keybindings {
        check,
        action,
        context,
        no_edit,
    } = &cli.command
    {
        let path = keybindings::keybindings_path(&keybindings::config_home()?);
        if let Some(action) = action {
            println!("{}", keybindings::inspect(action, context)?);
        } else if *check {
            let loaded = keybindings::load_from(&path);
            println!(
                "{} bindings; {} warnings",
                loaded.bindings.len(),
                loaded.warnings.len()
            );
            for warning in loaded.warnings {
                println!("{}: {}", warning.severity, warning.message);
            }
        } else {
            let created = keybindings::create_template(&path)?;
            println!(
                "{} {}",
                if created { "Created" } else { "Existing" },
                path.display()
            );
            if !*no_edit {
                if let Some(editor) =
                    std::env::var_os("VISUAL").or_else(|| std::env::var_os("EDITOR"))
                {
                    let status = std::process::Command::new(editor).arg(&path).status()?;
                    if !status.success() {
                        bail!("Editor exited with {status}");
                    }
                }
            }
        }
        return Ok(());
    }
    if let Commands::Init = &cli.command {
        let path = Config::init()?;
        println!("Created {}", path.display());
        return Ok(());
    }
    if let Commands::Stats = &cli.command {
        let root = canonical_workspace(&cli.workspace)?;
        println!("{}", stats::collect(&root)?.render());
        return Ok(());
    }
    if let Commands::Doctor = &cli.command {
        let root = canonical_workspace(&cli.workspace)?;
        match Config::load() {
            Ok(config) => println!(
                "{}",
                diagnostics::render(&diagnostics::inspect(&root, Some(&config)))
            ),
            Err(error) => {
                println!(
                    "{}",
                    diagnostics::render(&diagnostics::inspect(&root, None))
                );
                println!("[WARN] Configuration: {error:#}");
            }
        }
        return Ok(());
    }
    if let Commands::RenderAnsiSvg { text, options } = &cli.command {
        let input = match text {
            Some(value) => value.clone(),
            None => {
                let mut value = String::new();
                io::stdin().read_to_string(&mut value)?;
                value
            }
        };
        let opts = match options {
            Some(raw) => serde_json::from_str::<Value>(raw)?,
            None => json!({}),
        };
        let input = portable_render_semantics::expand_tabs(&input, 8);
        println!("{}", source_semantics::ansi_to_svg(&input, &opts));
        return Ok(());
    }
    if let Commands::InspectSource {
        source,
        export,
        args,
    } = &cli.command
    {
        if let Some(name) = export {
            let args: Vec<serde_json::Value> = match args {
                Some(raw) => serde_json::from_str(raw)
                    .map_err(|err| anyhow::anyhow!("--args must be a JSON array: {err}"))?,
                None => Vec::new(),
            };
            match source_semantics::inspect(source, name, &args)
                .or_else(|| source_semantics_osc::inspect(source, name, &args))
                .or_else(|| source_semantics_types::inspect(source, name, &args))
                .or_else(|| portable_render_semantics::inspect(source, name, &args))
                .or_else(|| platform_source_utilities::inspect(source, name, &args))
                .or_else(|| generated_ports::inspect(source, name, &args))
                .or_else(|| {
                    if source != "src/utils/transcriptSearch.ts" {
                        return None;
                    }
                    let value = args.first().unwrap_or(&Value::Null);
                    match name.as_str() {
                        "renderableSearchText" => {
                            Some(json!(transcript_search::renderable_search_text(value)))
                        }
                        "toolUseSearchText" => {
                            Some(json!(transcript_search::tool_use_search_text(value)))
                        }
                        "toolResultSearchText" => {
                            Some(json!(transcript_search::tool_result_search_text(value)))
                        }
                        _ => None,
                    }
                }) {
                Some(value) => {
                    if name == "view" {
                        if let Ok(tree) =
                            serde_json::from_value::<generated_ports::ui::VNode>(value.clone())
                        {
                            let missing = tree.unresolved_node_count();
                            if missing > 0 {
                                eprintln!("WARNING: {missing} unresolved JSX bindings; this view is a partial AST projection, not a runnable UI component");
                            }
                        }
                    }
                    println!("{}", serde_json::to_string_pretty(&value)?)
                }
                None => bail!("Source export unavailable: {source}::{name}"),
            }
        } else {
            let entries = generated_ports::catalog();
            let mut exports = entries
                .into_iter()
                .find(|(s, _)| *s == source)
                .map(|(_, exports)| exports)
                .unwrap_or_default();
            if let Some(source_exports) = source_semantics::exports(source) {
                for name in source_exports {
                    if !exports.contains(name) {
                        exports.push(*name);
                    }
                }
            }
            if let Some(source_exports) = source_semantics_osc::exports(source) {
                for name in source_exports {
                    if !exports.contains(name) {
                        exports.push(*name);
                    }
                }
            }
            if let Some(source_exports) = source_semantics_types::exports(source) {
                for name in source_exports {
                    if !exports.contains(name) {
                        exports.push(*name);
                    }
                }
            }
            if let Some(names) = platform_source_utilities::exports(source) {
                for name in names {
                    if !exports.contains(name) {
                        exports.push(*name);
                    }
                }
            }
            if let Some(names) = portable_render_semantics::exports(source) {
                for name in names {
                    if !exports.contains(name) {
                        exports.push(*name);
                    }
                }
            }
            if source == "src/utils/transcriptSearch.ts" {
                for name in [
                    "renderableSearchText",
                    "toolUseSearchText",
                    "toolResultSearchText",
                ] {
                    exports.push(name);
                }
            }
            if exports.is_empty() {
                bail!("Unknown migrated source: {source}");
            }
            for name in exports {
                println!("{name}")
            }
        }
        return Ok(());
    }
    if matches!(
        &cli.command,
        Commands::Sessions { .. }
            | Commands::Rename { .. }
            | Commands::Export { .. }
            | Commands::TranscriptSearch { .. }
    ) {
        let root = canonical_workspace(&cli.workspace)?;
        match &cli.command {
            Commands::Sessions { query } => {
                for item in session_control::search(&root, query.as_deref().unwrap_or(""))? {
                    println!("{}  {}  ({})", item.id, item.title, item.display_metadata());
                }
                return Ok(());
            }
            Commands::TranscriptSearch { session_id, query } => {
                let session = session_control::resolve(&root, session_id)?;
                for hit in
                    transcript_search::TranscriptIndex::from_stored(&session.messages).find(query)
                {
                    println!("{}  {}  {}", hit.message_index, hit.role, hit.excerpt);
                }
                return Ok(());
            }
            Commands::Rename { session_id, title } => {
                println!(
                    "{}",
                    session_control::rename_saved(&root, session_id, title)?
                );
                return Ok(());
            }
            Commands::Export {
                session_id,
                filename,
            } => {
                let path = session_control::export_saved(&root, session_id, filename.as_deref())?;
                println!("Conversation exported to: {}", path.display());
                return Ok(());
            }
            _ => {}
        }
    }
    let mut config = Config::load()?;
    if let Commands::Models { action } = &cli.command {
        if let Some(action) = action {
            model_catalog::run(action, &mut config).await?;
            return Ok(());
        }
        for profile in &config.providers {
            let default = if profile.id == config.default_provider {
                " (default)"
            } else {
                ""
            };
            println!(
                "{}{}: {} / {} @ {}",
                profile.id, default, profile.kind, profile.model, profile.base_url
            );
        }
        return Ok(());
    }
    if let Commands::SemanticSessions { query } = &cli.command {
        let root = canonical_workspace(&cli.workspace)?;
        let profile = config.select(cli.provider.as_deref(), cli.model.as_deref())?;
        let provider = provider::create(&profile)?;
        let (matches, _) =
            semantic_session_search::search(&root, query, provider.as_ref(), &profile.model)
                .await?;
        for item in matches {
            println!("{}  {}  ({})", item.id, item.title, item.display_metadata());
        }
        return Ok(());
    }
    if let Commands::Trigger { action } = &cli.command {
        let workspace = canonical_workspace(&cli.workspace)?;
        match action {
            LocalTriggerAction::Set { name, prompt } => {
                local_triggers::set(&workspace, name, &prompt.join(" "))?;
                println!("Saved local trigger: {name}");
                return Ok(());
            }
            LocalTriggerAction::Create { name, prompt } => {
                local_triggers::create(&workspace, name, &prompt.join(" "))?;
                println!("Created local trigger: {name}");
                return Ok(());
            }
            LocalTriggerAction::Update { name, prompt } => {
                local_triggers::change(&workspace, name, &prompt.join(" "))?;
                println!("Updated local trigger: {name}");
                return Ok(());
            }
            LocalTriggerAction::Get { name } => {
                println!("{}", local_triggers::get(&workspace, name)?);
                return Ok(());
            }
            LocalTriggerAction::List => {
                for name in local_triggers::list(&workspace)? {
                    println!("{name}");
                }
                return Ok(());
            }
            LocalTriggerAction::Delete { name } => {
                local_triggers::delete(&workspace, name)?;
                println!("Deleted local trigger: {name}");
                return Ok(());
            }
            LocalTriggerAction::Run { .. } => {}
        }
    }
    if cli.max_steps == 0 {
        bail!("--max-steps must be at least 1")
    }
    let root = canonical_workspace(&cli.workspace)?;
    if github_repo_mapping::update_repo_path_mapping(&mut config, &root)? {
        config.save()?;
    }
    let permission_paths = permission_settings::SettingsPaths::for_workspace(&root)?;
    let active_permission_rules = permission_paths.load(&config.permission_rules)?;
    let configured_default_mode = permission_paths.default_mode()?;
    let bypass_disabled = permission_paths.bypass_disabled()?;
    let permission_directories = permission_paths.additional_directories()?;
    let plugins = PluginSet::load(&root, &config.enabled_plugins)?;
    // Keep source-style per-plugin diagnostics visible. A broken plugin is
    // isolated from healthy ones, but its failure must never be silent.
    for warning in &plugins.errors {
        eprintln!("Plugin load warning: {warning}");
    }
    let profile = config.select(cli.provider.as_deref(), cli.model.as_deref())?;
    let interactive = matches!(&cli.command, Commands::Chat { .. } | Commands::ProjectInit);
    let mut session = match &cli.command {
        Commands::Chat {
            resume: Some(id), ..
        } => session_control::resolve(&root, id)?,
        Commands::Sdk {
            resume: Some(id), ..
        } => Session::load(&root, id)?,
        _ => Session::fresh(),
    };
    // A managed/user disable flag is stronger than a CLI bypass request and
    // also prevents a previously saved bypass mode from being restored.
    if bypass_disabled && session.permission_mode.as_deref() == Some("bypassPermissions") {
        session.permission_mode = Some("default".to_owned());
    }
    let has_restored_mode = session.permission_mode.is_some();
    let effective_approval = if bypass_disabled && matches!(cli.approval, ApprovalArg::Full) {
        Approval::Ask
    } else {
        cli.approval.into()
    };
    let mut agent = Agent::new(
        profile,
        root,
        effective_approval,
        interactive,
        cli.max_steps,
        session,
    )?;
    // CLI authorization ceilings outrank settings. A fresh Ask session may
    // adopt a configured ordinary mode; resume keeps its persisted mode.
    if !has_restored_mode && matches!(cli.approval, ApprovalArg::Ask) {
        if let Some(mode) = configured_default_mode {
            if matches!(
                mode,
                permission_mode::Mode::Default
                    | permission_mode::Mode::AcceptEdits
                    | permission_mode::Mode::Plan
                    | permission_mode::Mode::DontAsk
            ) {
                agent.set_sdk_permission_mode(mode.label())?;
            }
        }
    }
    agent.attach_configured_directories(&config.additional_directories)?;
    agent.attach_configured_directories(&permission_directories)?;
    for directory in &cli.additional_directory {
        let raw = directory.to_string_lossy();
        match agent.validate_workspace_directory(raw.as_ref())? {
            workspace_dirs::Validation::Success(path) => agent.add_workspace_directory(path),
            workspace_dirs::Validation::AlreadyAccessible { .. } => {}
            _ => bail!("Invalid --add-dir directory: {}", directory.display()),
        }
    }
    agent.set_default_shell(shell_runtime::from_config(config.default_shell.as_deref())?);
    agent.set_enabled_plugins(config.enabled_plugins.clone());
    agent.set_model_prices(&config.model_prices);
    agent.set_permission_rules(&active_permission_rules)?;
    agent.set_search_backend(config.search_backend.clone());
    let mut mcp_profiles = mcp_cli::active_servers(agent.workspace_root(), &config)?;
    mcp_profiles.extend(plugins.mcp_servers);
    agent.attach_mcp(&mcp_profiles).await?;
    agent.attach_lsp(&config.lsp_servers).await?;
    let mut hooks = config.hooks.clone();
    hooks.extend(plugins.hooks);
    agent.attach_hooks(&hooks).await?;
    if matches!(&cli.command, Commands::Sdk { .. }) {
        agent.set_structured_output(true);
    }
    eprintln!("Session: {}", agent.session_id());
    eprintln!(
        "Provider: {} / {}",
        agent.profile().id,
        agent.profile().model
    );
    let result = match &cli.command {
        Commands::Review { pr_number } => {
            let prompt = git_workflows::prompt(
                git_workflows::Action::Review,
                agent.workspace_root(),
                &pr_number.map(|n| n.to_string()).unwrap_or_default(),
            )?;
            agent.ask(&prompt).await.map(|answer| println!("{answer}"))
        }
        Commands::UltraReview { pr_number } => {
            let prompt =
                local_advanced_workflows::review_prompt(agent.workspace_root(), *pr_number)?;
            agent
                .ask_in_plan_mode(&prompt)
                .await
                .map(|answer| println!("{answer}"))
        }
        Commands::UltraPlan { objective } => {
            let prompt = local_advanced_workflows::plan_prompt(
                agent.workspace_root(),
                &objective.join(" "),
            )?;
            agent
                .ask_in_plan_mode(&prompt)
                .await
                .map(|answer| println!("{answer}"))
        }
        Commands::Trigger {
            action: LocalTriggerAction::Run { name },
        } => {
            let prompt = local_triggers::get(agent.workspace_root(), name)?;
            agent.ask(&prompt).await.map(|answer| println!("{answer}"))
        }
        Commands::Trigger { .. } => {
            unreachable!("Local trigger management is handled before model creation")
        }
        Commands::Commit { instructions } => {
            let prompt = git_workflows::prompt(
                git_workflows::Action::Commit,
                agent.workspace_root(),
                instructions.as_deref().unwrap_or(""),
            )?;
            agent.ask(&prompt).await.map(|answer| println!("{answer}"))
        }
        Commands::CommitPushPr { instructions } => {
            let prompt = git_workflows::prompt(
                git_workflows::Action::CommitPushPr,
                agent.workspace_root(),
                instructions.as_deref().unwrap_or(""),
            )?;
            agent.ask(&prompt).await.map(|answer| println!("{answer}"))
        }
        Commands::Run { prompt } => {
            let actual_prompt = if prompt == "-" {
                let mut input = String::new();
                io::stdin().read_to_string(&mut input)?;
                input
            } else {
                prompt.clone()
            };
            agent
                .ask(&actual_prompt)
                .await
                .map(|answer| println!("{answer}"))
        }
        Commands::ProjectInit => {
            let prompt = project_init::prompt(agent.workspace_root())?;
            agent.ask(&prompt).await.map(|answer| println!("{answer}"))
        }
        Commands::Chat {
            prefill,
            deep_link_origin,
            deep_link_repo,
            deep_link_last_fetch,
            ..
        } => {
            chat(
                &mut agent,
                &mut config,
                prefill.as_deref(),
                *deep_link_origin,
                deep_link_repo.as_deref(),
                *deep_link_last_fetch,
            )
            .await
        }
        Commands::Sdk { .. } => sdk(&mut agent, &config).await,
        _ => unreachable!(),
    };
    let end_result = agent.finish().await;
    result?;
    end_result
}

/// Resolve a real user-invocable skill or plugin command. Shared by the
/// interactive CLI and SDK so the same frontmatter restrictions apply.
pub(crate) fn expand_user_skill_prompt(
    agent: &Agent,
    _config: &Config,
    command: &str,
    args: &str,
    is_mcp: bool,
) -> Result<Option<String>> {
    if is_mcp {
        return Ok(None);
    }
    if command.contains(':') {
        return plugins::command_prompt_with_session(
            agent.workspace_root(),
            agent.enabled_plugins(),
            command,
            args,
            Some(&agent.session_id()),
        );
    }
    if !command.is_empty()
        && command.len() <= 64
        && command
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        && agent
            .workspace_root()
            .join(".atlas/skills")
            .join(command)
            .join("SKILL.md")
            .is_file()
    {
        return agent.invoke_user_skill(command, args).map(Some);
    }
    Ok(None)
}

/// Reload the live plugin runtime and the CLI's skill/command catalog as one
/// operation. The current conversation and model remain untouched.
async fn reload_live_plugins(agent: &mut Agent, config: &mut Config) -> Result<Value> {
    let refreshed = Config::load()?;
    let status = agent.reload_plugins(&refreshed).await?;
    config.enabled_plugins = refreshed.enabled_plugins;
    Ok(status)
}

/// The original AgentEditor/creation wizard surfaces non-blocking
/// validation warnings and requires the user to accept them before saving.
fn confirm_custom_agent_warnings(
    definition: &agent_definition::AgentDefinition,
    workspace: &std::path::Path,
    user: bool,
) -> Result<bool> {
    let validation = agent_definition::validate_in_workspace(definition, workspace, user)?;
    if !validation.errors.is_empty() {
        bail!("{}", validation.errors.join("; "));
    }
    if validation.warnings.is_empty() {
        return Ok(true);
    }
    for warning in &validation.warnings {
        eprintln!("Agent warning: {warning}");
    }
    print!("Save agent despite these warnings? [y/N]: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer)? == 0 {
        return Ok(false);
    }
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

async fn chat(
    agent: &mut Agent,
    config: &mut Config,
    prefill: Option<&str>,
    deep_link_origin: bool,
    deep_link_repo: Option<&str>,
    last_fetch_ms: Option<u64>,
) -> Result<()> {
    let mut prompt_history =
        prompt_history::PromptHistory::new(agent.workspace_root(), &agent.session_id())?;
    println!("Atlas Agent. /help lists commands; /exit quits.");
    let mut onboarding = project_onboarding::Onboarding::for_workspace(agent.workspace_root())?;
    if onboarding.should_show()? {
        if let Some(suggestion) = onboarding.current_suggestion()? {
            println!("Project setup: {suggestion}");
        }
        onboarding.mark_seen()?;
    }
    if deep_link_origin {
        let last_fetch = last_fetch_ms.and_then(|value| {
            std::time::SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_millis(value))
        });
        let banner = deep_link_banner::DeepLinkBannerInfo {
            cwd: agent.workspace_root().to_path_buf(),
            prefill_length: prefill.map(str::chars).map(Iterator::count),
            repo: deep_link_repo.map(str::to_owned),
            last_fetch,
        };
        println!("{}", deep_link_banner::build_deep_link_banner(&banner));
    }
    let mut initial_prefill = prefill.map(str::to_owned);
    loop {
        let prompt = if let Some(ref suggested) = initial_prefill {
            println!("Proposed prompt: {suggested}");
            "atlas [Enter: use linked prompt; or type replacement]> ".to_owned()
        } else {
            use std::io::IsTerminal as _;
            session_color::prompt_prefix(agent.session_color(), io::stdout().is_terminal())
        };
        let entries: Vec<String> = prompt_history
            .get_history()?
            .into_iter()
            .map(|entry| entry.display)
            .collect();
        let line =
            match terminal_input::read_prompt(&prompt, &entries, config.editor_mode == "vim")? {
                terminal_input::PromptRead::Line(line) => line,
                terminal_input::PromptRead::Cancelled => continue,
                terminal_input::PromptRead::Eof => break,
            };
        let entry = if line.trim().is_empty() {
            initial_prefill.take().unwrap_or_default()
        } else {
            initial_prefill = None;
            line
        };
        let input = entry.trim();
        if input.is_empty() {
            continue;
        }
        // Canonical alias resolution uses original command index descriptors.
        let normalized = input.strip_prefix('/').and_then(|tail| {
            let name = tail.split_once(' ').map(|(x, _)| x).unwrap_or(tail);
            let command = command_manifest::lookup(name)?;
            if command.name == name {
                return None;
            }
            let rest = &tail[name.len()..];
            Some(format!("/{}{}", command.name, rest))
        });
        let input = normalized.as_deref().unwrap_or(input);
        if input == "/advisor" || input.starts_with("/advisor ") {
            let args = input.strip_prefix("/advisor").unwrap_or("").trim();
            match advisor_command::execute(config, agent.profile(), args) {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Cannot change advisor model: {error:#}"),
            }
            continue;
        }
        if input == "/color" || input.starts_with("/color ") {
            let color = input.strip_prefix("/color").unwrap_or("").trim();
            match agent.set_session_color(color) {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Cannot set session color: {error:#}"),
            }
            continue;
        }
        if input == "/keybindings" || input.starts_with("/keybindings ") {
            let arg = input.strip_prefix("/keybindings").unwrap_or("").trim();
            let path = keybindings::keybindings_path(&keybindings::config_home()?);
            if arg == "check" {
                let loaded = keybindings::load_from(&path);
                println!(
                    "{} bindings; {} warnings",
                    loaded.bindings.len(),
                    loaded.warnings.len()
                );
                for w in loaded.warnings {
                    println!("{}: {}", w.severity, w.message);
                }
            } else if arg.is_empty() {
                let created = keybindings::create_template(&path)?;
                println!(
                    "{} {}",
                    if created { "Created" } else { "Existing" },
                    path.display()
                );
                if let Some(editor) =
                    std::env::var_os("VISUAL").or_else(|| std::env::var_os("EDITOR"))
                {
                    let status = std::process::Command::new(editor).arg(&path).status()?;
                    if !status.success() {
                        eprintln!("Editor exited with {status}");
                    }
                }
            } else {
                println!("{}", keybindings::inspect(arg, "Chat")?);
            }
            continue;
        }
        if let Some(command) = input.strip_prefix("/permission ") {
            let outcome = (|| -> Result<String> {
                let mut parts = command.splitn(4, ' ');
                let action = parts.next().unwrap_or("");
                let source = match parts.next().unwrap_or("") {
                    "user" => permission_settings::EditableSource::User,
                    "project" => permission_settings::EditableSource::Project,
                    "local" => permission_settings::EditableSource::Local,
                    _ => bail!("Source must be user, project, or local"),
                };
                let behavior = match parts.next().unwrap_or("") {
                    "allow" => permission_rules::Behavior::Allow,
                    "deny" => permission_rules::Behavior::Deny,
                    "ask" => permission_rules::Behavior::Ask,
                    _ => bail!("Behavior must be allow, deny, or ask"),
                };
                let raw_rule = parts.next().unwrap_or("").trim();
                if raw_rule.is_empty() {
                    bail!("Missing permission rule, e.g. Bash(git status)");
                }
                let rule = permission_rules::parse_rule(raw_rule);
                let paths =
                    permission_settings::SettingsPaths::for_workspace(agent.workspace_root())?;
                let changed = match action {
                    "add" => paths.add(source, behavior, &[rule])?,
                    "delete" => paths.delete(source, behavior, &rule)?,
                    _ => bail!("Action must be add or delete"),
                };
                let active = paths.load(&config.permission_rules)?;
                agent.set_permission_rules(&active)?;
                Ok(format!(
                    "Permission {}: {}",
                    if changed {
                        "updated"
                    } else {
                        "unchanged or policy-locked"
                    },
                    raw_rule
                ))
            })();
            match outcome {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Permission update failed: {error:#}"),
            }
            continue;
        }
        if input == "/find" || input.starts_with("/find ") {
            let query = input.strip_prefix("/find").unwrap_or("").trim();
            if query.is_empty() {
                eprintln!("Usage: /find TEXT");
            } else {
                for hit in agent.search_transcript(query) {
                    println!("{}  {}  {}", hit.message_index, hit.role, hit.excerpt);
                }
            }
            continue;
        }
        if input == "/plugin" || input.starts_with("/plugin ") {
            let args = input.strip_prefix("/plugin").unwrap_or("").trim();
            let command = plugin_commands::parse_plugin_args(Some(args));
            // Keep the source's interactive subcommands on the same installer,
            // managed-policy gate, dependency transaction and error path as CLI.
            let changed = match &command {
                plugin_commands::PluginCommand::Install { .. }
                | plugin_commands::PluginCommand::Uninstall { .. }
                | plugin_commands::PluginCommand::Enable { .. }
                | plugin_commands::PluginCommand::Disable { .. } => true,
                plugin_commands::PluginCommand::Marketplace { action, .. } => {
                    action.as_deref() != Some("list")
                }
                _ => false,
            };
            match plugin_manager::execute(agent.workspace_root(), &command) {
                Ok(output) => {
                    println!("{}", serde_json::to_string_pretty(&output)?);
                    if changed {
                        match reload_live_plugins(agent, config).await {
                            Ok(status) => println!(
                                "Live plugin runtime refreshed: {}",
                                serde_json::to_string(&status)?
                            ),
                            Err(error) => eprintln!(
                                "Plugin command completed, but the live session could not reload: {error:#}. Run /reload-plugins to retry."
                            ),
                        }
                    }
                }
                Err(error) => eprintln!("Plugin command failed: {error:#}"),
            }
            continue;
        }
        if input == "/reload-plugins" {
            // The source /reload-plugins command refreshes the live session,
            // without restarting the model or dropping conversation history.
            match reload_live_plugins(agent, config).await {
                Ok(status) => {
                    println!(
                        "Reloaded plugin hooks and MCP services ({} configured plugins).",
                        config.enabled_plugins.len()
                    );
                    println!("{}", serde_json::to_string_pretty(&status)?);
                    match plugins::list_commands(agent.workspace_root(), &config.enabled_plugins) {
                        Ok(commands) => {
                            println!("Available plugin commands and skills: {}", commands.len())
                        }
                        Err(error) => {
                            eprintln!("Plugin command discovery failed after reload: {error:#}")
                        }
                    }
                }
                Err(error) => eprintln!("Plugin reload failed: {error:#}"),
            }
            continue;
        }
        if input == "/history" || input.starts_with("/history ") {
            let query = input
                .strip_prefix("/history")
                .unwrap_or("")
                .trim()
                .to_lowercase();
            for item in prompt_history
                .get_timestamped_history()?
                .into_iter()
                .filter(|entry| entry.display.to_lowercase().contains(&query))
                .take(20)
            {
                println!("{}  {}", item.timestamp, item.display.replace('\n', "\\n"));
            }
            continue;
        }
        // Record submissions before model execution, including failed requests.
        // History persistence is best-effort in 2.1.88: a locked or full
        // history file must not discard the user's actual chat submission.
        if let Err(error) = prompt_history.add(input, std::collections::BTreeMap::new()) {
            eprintln!("Prompt history could not be saved: {error:#}");
        }
        if let Some(direct) = portable_missing_utilities::parse_direct_member_message(input) {
            let result = agent.send_direct_user_message(&direct.recipient, &direct.message)?;
            println!("{}", serde_json::to_string(&result)?);
            continue;
        }
        if let Some(name) = input.strip_prefix("/shell-spec ") {
            match portable_missing_utilities::get_builtin_command_spec(name.trim()) {
                Some(spec) => println!("{}", serde_json::to_string_pretty(&spec)?),
                None => eprintln!("No registered shell command spec: {}", name.trim()),
            }
            continue;
        }
        if let Some(raw) = input.strip_prefix("/mcp ") {
            use generated_ports::p_7522da9606aa2::{
                resolve_toggle, route_args, toggle_enabled_state, McpClient, McpRoute,
                McpToggleOutcome,
            };
            // Keep the existing manual reconnect-failed shortcut; named
            // reconnects and all enable/disable actions use the 2.1.88
            // /mcp command route and target selection.
            if raw.trim() != "reconnect" {
                match route_args(Some(raw)) {
                    McpRoute::Reconnect(name) => match agent.reconnect_named_mcp(&name).await {
                        Ok(status) => println!("{}", serde_json::to_string_pretty(&status)?),
                        Err(error) => eprintln!("MCP reconnect failed: {error:#}"),
                    },
                    McpRoute::Toggle { action, target } => {
                        let clients = agent
                            .mcp_statuses()?
                            .iter()
                            .filter_map(|value| {
                                Some(McpClient {
                                    name: value.get("name")?.as_str()?.to_owned(),
                                    ctype: value.get("status")?.as_str()?.to_owned(),
                                })
                            })
                            .collect::<Vec<_>>();
                        let (outcome, names) = resolve_toggle(&action, &target, &clients);
                        let mut succeeded = 0;
                        for name in &names {
                            let client = clients
                                .iter()
                                .find(|client| &client.name == name)
                                .with_context(|| format!("MCP toggle target {name:?} vanished"))?;
                            let target_enabled = toggle_enabled_state(&action, &target, client);
                            match agent.toggle_named_mcp(name, target_enabled).await {
                                Ok(_) => succeeded += 1,
                                Err(error) => eprintln!("MCP {action} {name} failed: {error:#}"),
                            }
                        }
                        match outcome {
                            McpToggleOutcome::AlreadyAll(message) => println!("{message}"),
                            McpToggleOutcome::NotFound(message) => println!("{message}"),
                            McpToggleOutcome::ToggledAll { enabled, .. } => {
                                let action = if enabled { "Enabled" } else { "Disabled" };
                                println!("{action} {succeeded} MCP server(s)");
                            }
                            McpToggleOutcome::ToggledOne { name, enabled } if succeeded == 1 => {
                                let state = if enabled { "enabled" } else { "disabled" };
                                println!("MCP server \"{name}\" {state}");
                            }
                            McpToggleOutcome::ToggledOne { .. } => {}
                        }
                    }
                    McpRoute::Settings => {
                        for status in agent.mcp_statuses()? {
                            println!("{}", serde_json::to_string_pretty(&status)?);
                        }
                    }
                }
                continue;
            }
        }
        if let Some(raw) = input.strip_prefix("/mode set ") {
            let raw = raw.trim();
            let remember = raw.ends_with(" --remember");
            let label = if remember {
                raw.trim_end_matches(" --remember").trim()
            } else {
                raw
            };
            let Some(mode) = permission_mode::Mode::parse(label) else {
                eprintln!("Unknown permission mode: {label}");
                continue;
            };
            agent.set_sdk_permission_mode(mode.label())?;
            if remember {
                let paths =
                    permission_settings::SettingsPaths::for_workspace(agent.workspace_root())?;
                match paths.set_default_mode(permission_settings::EditableSource::Local, mode) {
                    Ok(true) => {
                        println!("Permission mode: {} (saved as local default)", mode.label())
                    }
                    Ok(false) => eprintln!(
                        "Permission mode: {} (managed policy blocks saving)",
                        mode.label()
                    ),
                    Err(error) => eprintln!(
                        "Permission mode: {} (local settings save failed: {error:#})",
                        mode.label()
                    ),
                }
            } else {
                println!("Permission mode: {}", mode.label());
            }
            continue;
        }
        if let Some(raw) = input.strip_prefix("/dirs remove ") {
            let raw = raw.trim();
            if raw.is_empty() {
                eprintln!("Usage: /dirs remove PATH");
                continue;
            }
            let supplied = std::path::Path::new(raw);
            let resolved = if supplied.is_absolute() {
                supplied.to_path_buf()
            } else {
                agent.workspace_root().join(supplied)
            };
            let resolved = resolved.canonicalize().unwrap_or(resolved);
            if resolved == agent.workspace_root() {
                eprintln!("Cannot remove the primary workspace");
                continue;
            }
            let removed = agent.remove_workspace_directory(&resolved);
            if removed {
                println!("Revoked {} for this session", resolved.display());
            } else {
                println!("{} is not an active extra directory", resolved.display());
            }
            let paths = permission_settings::SettingsPaths::for_workspace(agent.workspace_root())?;
            match paths.remove_directory(permission_settings::EditableSource::Local, &resolved) {
                Ok(true) => println!("Removed {} from local settings", resolved.display()),
                Ok(false) => println!("No removable local setting for {}", resolved.display()),
                Err(error) => {
                    eprintln!("Revoked for this session; local settings update failed: {error:#}")
                }
            }
            continue;
        }
        match input {
            "/exit" | "/quit" => break,
            "/help" => {
                println!("/model PROVIDER [MODEL]  switch provider or model");
                println!("/session                 show resumable session id");
                println!(
                    "/status                  show live session, model, permissions and MCP status"
                );
                println!(
                    "/doctor                  run local installation and configuration diagnostics"
                );
                println!("/tag NAME                add/replace a searchable session tag; repeat to remove");
                println!("/branch [name]           fork the conversation and resume independently");
                println!("/init                    set up project instructions, optional skills and hooks");
                println!("/stats                   report real local session usage and costs");
                println!("/release-notes           show trusted cached release notes");
                println!("/review [PR]             review a pull request through the active model");
                println!("/ultrareview [PR]        deep local multi-angle code review");
                println!("/ultraplan GOAL          detailed local read-only project planning");
                println!("/trigger create NAME GOAL  create a new local task");
                println!("/trigger get NAME          read a saved task");
                println!("/trigger update NAME GOAL  update an existing local task");
                println!("/trigger set NAME GOAL     save or replace a task for any model");
                println!("/trigger run NAME        execute saved local task now");
                println!("/trigger list|delete NAME  inspect or remove saved local tasks");
                println!("/commit                  review changes and create a Git commit");
                println!("/commit-push-pr          commit, push and create/update a Chinese PR");
                println!("/history [query]         search saved prompt history");
                println!(
                    "/files                   show files read successfully in this conversation"
                );
                println!("/copy [N] [--code K]     choose full response or a Markdown code block");
                println!("/copy --reset            restore interactive copy selection");
                println!(
                    "/diff                    view staged, unstaged and untracked Git changes"
                );
                println!("/skills                  list accessible project and plugin skills");
                println!("/plugin [subcommand]     manage, install, enable, disable and validate plugins");
                println!(
                    "/reload-plugins          refresh active plugin hooks, MCP and command catalog"
                );
                println!("/agents                  list project and user custom agent definitions");
                println!("/agents show project|user NAME    inspect a custom agent definition");
                println!("/agents create project|user NAME  create a custom agent interactively");
                println!(
                    "/agents generate project|user [DESCRIPTION]  generate and review an agent"
                );
                println!(
                    "/agents delete project|user NAME  delete a custom agent with confirmation"
                );
                println!(
                    "/agents edit project|user NAME    replace an existing agent after validation"
                );
                println!("/hooks                   list active hook events, matchers and timeouts");
                println!("/find TEXT               search visible messages in this conversation");
                println!("Offline: atlas-agent transcript-search SESSION_ID TEXT");
                println!("/permission add|delete user|project|local allow|deny|ask RULE");
                println!("/permissions             inspect persisted rules by source");
                println!("/sessions [query]        search saved conversation titles");
                println!("/sessions --semantic QUERY use model-ranked session search");
                println!("One-shot model search: atlas-agent semantic-sessions QUERY");
                println!("/resume [TERM]           pick or directly resume a saved conversation");
                println!("/rename [NAME]           rename conversation or generate a name using active model");
                println!("/export [filename]       save full conversation to a local text file");
                println!("Resume: atlas-agent chat --resume SESSION_ID");
                println!("/usage                   show provider-reported token totals");
                println!("/cost                    show model costs and unknown pricing");
                println!("/context                 show live model context usage by category");
                println!("/version                 show version of this running binary");
                println!("/tasks                   list background shell tasks");
                println!(
                    "/mode                    cycle permission mode (default/accept edits/plan)"
                );
                println!("/mode set MODE [--remember] switch mode and optionally save default");
                println!("/add-dir PATH [--remember] add an authorized working directory");
                println!("/dirs                    show authorized working directories");
                println!("/dirs remove PATH        revoke a directory and forget local permission");
                println!("/env [NAME=VALUE | -u NAME] configure environment for child commands");
                println!("/clear                   erase current conversation history");
                println!("/compact                 summarize and compact conversation history");
                println!("/commands                list migrated command registration metadata");
                println!("@teammate MESSAGE        send directly to active team mailbox");
                println!("/shell-spec [name]       show registered shell command specs");
                println!("/mcp [status|reconnect]  inspect or recover MCP connections");
                println!("/mcp reconnect SERVER_NAME  reconnect only the named MCP server");
                println!("/mcp enable|disable SERVER_NAME  persistently toggle one MCP server");
                println!("/mcp-prompts             list available MCP prompt commands");
                println!("/server:prompt (MCP) ARGS  execute an MCP prompt");
                println!("/exit                    quit");
                continue;
            }
            "/shell-spec" => {
                for spec in portable_missing_utilities::builtin_command_specs() {
                    println!("{} — {}", spec.name, spec.description);
                    for argument in spec.arguments {
                        println!("  {} — {}", argument.name, argument.description);
                    }
                }
                continue;
            }
            "/commands" => {
                println!("Command registrations (handlers may still be pending Rust migration):");
                for entry in command_manifest::visible() {
                    let alias = if entry.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", entry.aliases.join(", "))
                    };
                    println!("/{}{}  {}", entry.name, alias, entry.description);
                }
                match plugins::command_descriptors(agent.workspace_root(), &config.enabled_plugins)
                {
                    Ok(commands) => {
                        for command in commands {
                            let name = command["name"].as_str().unwrap_or("");
                            let description = command["description"].as_str().unwrap_or("");
                            let hint = command["argumentHint"].as_str().unwrap_or("");
                            if hint.is_empty() {
                                println!("/{name}  {description}");
                            } else {
                                println!("/{name} {hint}  {description}");
                            }
                        }
                    }
                    Err(error) => eprintln!("Plugin command discovery failed: {error:#}"),
                }
                continue;
            }
            "/dirs" => {
                for dir in agent.working_directories() {
                    println!("{}", dir.display());
                }
                continue;
            }
            "/env" => {
                for (name, value) in agent.environment_variables() {
                    println!("{name}={value}");
                }
                continue;
            }
            "/session" => {
                println!("{}", agent.session_id());
                continue;
            }
            "/status" => {
                println!("{}", agent.status_report()?);
                continue;
            }
            "/files" => {
                println!("{}", agent.files_in_context());
                continue;
            }
            "/context" => {
                println!("{}", agent.context_usage_text()?);
                continue;
            }
            "/permissions" | "/allowed-tools" => {
                let paths =
                    permission_settings::SettingsPaths::for_workspace(agent.workspace_root())?;
                println!(
                    "{}",
                    paths.render_permission_rules(agent.current_permission_mode())?
                );
                continue;
            }
            "/diff" => {
                match diff_view::render(agent.workspace_root()) {
                    Ok(output) => println!("{output}"),
                    Err(error) => eprintln!("Cannot inspect diff: {error:#}"),
                }
                continue;
            }
            "/skills" => {
                match agent.list_user_skills() {
                    Ok(skills) if skills.is_empty() => println!("No accessible skills."),
                    Ok(skills) => {
                        for name in skills.lines() {
                            let source = if name.contains(':') {
                                plugins::read_skill(
                                    agent.workspace_root(),
                                    &config.enabled_plugins,
                                    name,
                                )
                            } else {
                                agent
                                    .workspace_root()
                                    .join(".atlas/skills")
                                    .join(name)
                                    .join("SKILL.md")
                                    .canonicalize()
                                    .and_then(std::fs::read_to_string)
                                    .map_err(anyhow::Error::from)
                            };
                            match source {
                                Ok(source) => {
                                    let (description, hint) =
                                        skill_arguments::command_metadata(&source, "Project skill");
                                    if hint.is_empty() {
                                        println!("/{name}  {description}");
                                    } else {
                                        println!("/{name} {hint}  {description}");
                                    }
                                }
                                Err(error) => eprintln!("Cannot describe skill {name}: {error:#}"),
                            }
                        }
                    }
                    Err(error) => eprintln!("Cannot list skills: {error:#}"),
                }
                continue;
            }
            "/agents" => {
                match agent.list_agents() {
                    Ok(agents) => println!("{agents}"),
                    Err(error) => eprintln!("Cannot list agents: {error:#}"),
                }
                continue;
            }
            "/hooks" => {
                println!("{}", agent.hooks_report());
                continue;
            }

            "/release-notes" => {
                let home = keybindings::config_home()?;
                let url = std::env::var("ATLAS_RELEASE_NOTES_URL").ok();
                match release_notes::show(&home, url.as_deref()).await {
                    Ok(notes) => println!("{notes}"),
                    Err(error) => eprintln!("Cannot show release notes: {error:#}"),
                }
                continue;
            }
            "/stats" => {
                println!("{}", stats::collect(agent.workspace_root())?.render());
                continue;
            }

            "/tag" => {
                println!(
                    "Usage: /tag <tag-name> — add a searchable session tag; repeat to remove."
                );
                continue;
            }

            "/doctor" => {
                println!(
                    "{}",
                    diagnostics::render(&diagnostics::inspect(
                        agent.workspace_root(),
                        Some(config)
                    ))
                );
                for status in agent.mcp_statuses()? {
                    let name = status["name"].as_str().unwrap_or("unnamed");
                    let state = status["status"].as_str().unwrap_or("unknown");
                    println!("Live MCP: {name}: {state}");
                }
                continue;
            }
            "/sessions" => {
                for item in agent.list_sessions()? {
                    println!("{}  {}  ({})", item.id, item.title, item.display_metadata());
                }
                continue;
            }
            "/vim" => {
                println!("{}", vim_mode_source::toggle_editor_mode(config)?);
                continue;
            }
            "/mode" => {
                println!("Permission mode: {}", agent.cycle_permission_mode()?);
                continue;
            }
            "/tasks" => {
                println!("{}", agent.background_tasks()?);
                continue;
            }
            "/mcp" => {
                for status in agent.mcp_statuses()? {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                continue;
            }
            "/mcp reconnect" => {
                let failures = agent.reconnect_failed_mcp().await?;
                if failures.is_empty() {
                    println!("All recoverable MCP connections are available.");
                } else {
                    eprintln!("{} MCP connections still failed.", failures.len());
                }
                for status in agent.mcp_statuses()? {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                continue;
            }
            "/mcp-prompts" => {
                let catalog = agent.list_mcp_prompt_commands().await?;
                let cmds = catalog.as_array().expect("MCP commands are an array");
                if cmds.is_empty() {
                    println!("No MCP prompt commands are available.");
                }
                for cmd in cmds {
                    println!(
                        "/{} — {}",
                        cmd["userFacingName"].as_str().unwrap_or(""),
                        cmd["description"].as_str().unwrap_or("")
                    );
                }
                continue;
            }
            "/version" => {
                println!("{}", version::running_version());
                continue;
            }
            "/output-style" => {
                println!("{}", LEGACY_OUTPUT_STYLE_NOTICE);
                continue;
            }
            "/cost" => {
                println!("{}", agent.cost_summary());
                continue;
            }
            "/usage" => {
                let usage = agent.usage();
                println!(
                    "Input: {}  Output: {}  Total: {} tokens",
                    usage.input_tokens, usage.output_tokens, usage.total_tokens
                );
                continue;
            }
            "/clear" => {
                agent.clear_history().await?;
                // /clear starts a new UUID. Subsequent history entries must be
                // attributed to that new session rather than the old one.
                prompt_history = prompt_history::PromptHistory::new(
                    agent.workspace_root(),
                    &agent.session_id(),
                )?;
                println!("Conversation cleared. Session: {}", agent.session_id());
                continue;
            }
            "/compact" => {
                agent.compact_history().await?;
                println!(
                    "{}",
                    compact_command_source::build_display_text(false, "ctrl+o", None, None)
                );
                continue;
            }
            _ => {}
        }
        if let Some(instructions) = input.strip_prefix("/compact ") {
            agent.compact_history_custom(instructions).await?;
            println!(
                "{}",
                compact_command_source::build_display_text(false, "ctrl+o", None, None)
            );
            continue;
        }
        if input == "/sessions --semantic" || input.starts_with("/sessions --semantic ") {
            let query = input.trim_start_matches("/sessions --semantic").trim();
            if query.is_empty() {
                eprintln!("Usage: /sessions --semantic QUERY");
                continue;
            }
            match agent.semantic_search_sessions(query).await {
                Ok(matches) => {
                    for item in matches {
                        println!(
                            "{}  {}  ({} messages)",
                            item.id,
                            item.display_title(),
                            item.messages
                        );
                    }
                }
                Err(error) => eprintln!("Semantic session search failed: {error:#}"),
            }
            continue;
        }
        if input.starts_with("/sessions ") {
            for item in agent.search_sessions(input.trim_start_matches("/sessions").trim())? {
                println!(
                    "{}  {}  ({} messages)",
                    item.id,
                    item.display_title(),
                    item.messages
                );
            }
            continue;
        }
        if let Some(raw) = input.strip_prefix("/tag ") {
            let tag = crate::unicode_sanitization::partially_sanitize_unicode(raw);
            let tag = tag.trim();
            if tag.is_empty() || matches!(tag, "--help" | "-h" | "help") {
                println!("Usage: /tag <tag-name> — toggle a searchable session tag.");
                continue;
            }
            if agent.session_tag() == Some(tag) {
                print!("Current tag is #{tag}. Remove from session? [y/N]: ");
                io::stdout().flush()?;
                let mut response = String::new();
                io::stdin().read_line(&mut response)?;
                if matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    println!("{}", agent.set_session_tag(None)?);
                } else {
                    println!("Kept tag #{tag}");
                }
            } else {
                println!("{}", agent.set_session_tag(Some(tag))?);
            }
            continue;
        }
        if input == "/init" {
            let prompt = project_init::prompt(agent.workspace_root())?;
            match agent.ask(&prompt).await {
                Ok(answer) => println!("{answer}"),
                Err(error) => eprintln!("Project initialization failed: {error:#}"),
            }
            onboarding.maybe_mark_complete()?;
            continue;
        }
        if input.starts_with("/agents show ") {
            let args = input.trim_start_matches("/agents show ").trim();
            let outcome = (|| -> Result<String> {
                let parts = args.split_whitespace().collect::<Vec<_>>();
                let [scope, name] = parts.as_slice() else {
                    bail!("Usage: /agents show project|user NAME");
                };
                let user = match *scope {
                    "project" => false,
                    "user" => true,
                    _ => bail!("Agent location must be project or user"),
                };
                let path = agent_definition::new_file_path(agent.workspace_root(), user, name)?;
                let metadata = std::fs::symlink_metadata(&path)
                    .with_context(|| format!("Agent definition missing: {}", path.display()))?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    bail!("Agent definition must be a regular file");
                }
                let content = agent_definition::read_agent_file(&path)?;
                Ok(format!(
                    "{} agent {} ({})\n{}",
                    scope,
                    name,
                    path.display(),
                    content
                ))
            })();
            match outcome {
                Ok(details) => println!("{details}"),
                Err(error) => eprintln!("Agent details unavailable: {error:#}"),
            }
            continue;
        }
        if input.starts_with("/agents generate ") {
            let args = input.trim_start_matches("/agents generate ").trim();
            let mut pieces = args.splitn(2, ' ');
            let scope = pieces.next().unwrap_or("");
            let user = match scope {
                "project" => false,
                "user" => true,
                _ => {
                    eprintln!("Usage: /agents generate project|user [DESCRIPTION]");
                    continue;
                }
            };
            let supplied = pieces.next().unwrap_or("").trim();
            let request = if supplied.is_empty() {
                print!("Describe the agent to generate: ");
                io::stdout().flush()?;
                let mut line = String::new();
                if io::stdin().read_line(&mut line)? == 0 {
                    println!("Agent generation cancelled.");
                    continue;
                }
                line.trim().to_owned()
            } else {
                supplied.to_owned()
            };
            match agent.generate_custom_agent_definition(&request).await {
                Ok(definition) => {
                    println!("Generated agent: {}", definition.agent_type);
                    println!("When to use:\n{}", definition.when_to_use);
                    println!("System prompt:\n{}", definition.system_prompt);
                    let decision = (|| -> Result<bool> {
                        if !confirm_custom_agent_warnings(
                            &definition,
                            agent.workspace_root(),
                            user,
                        )? {
                            return Ok(false);
                        }
                        print!("Save generated agent {}? [y/N]: ", definition.agent_type);
                        io::stdout().flush()?;
                        let mut answer = String::new();
                        if io::stdin().read_line(&mut answer)? == 0 {
                            return Ok(false);
                        }
                        Ok(matches!(
                            answer.trim().to_ascii_lowercase().as_str(),
                            "y" | "yes"
                        ))
                    })();
                    match decision {
                        Ok(true) => match agent_definition::save_agent(
                            &definition,
                            agent.workspace_root(),
                            user,
                            false,
                        ) {
                            Ok(path) => println!("Created generated agent at {}", path.display()),
                            Err(error) => eprintln!("Cannot save generated agent: {error:#}"),
                        },
                        Ok(false) => println!("Generated agent discarded without saving."),
                        Err(error) => eprintln!("Agent validation failed: {error:#}"),
                    }
                }
                Err(error) => eprintln!("Agent generation failed: {error:#}"),
            }
            continue;
        }
        if input.starts_with("/agents create ") {
            let args = input.trim_start_matches("/agents create ").trim();
            let outcome = (|| -> Result<String> {
                let parts = args.split_whitespace().collect::<Vec<_>>();
                let [scope, name] = parts.as_slice() else {
                    bail!("Usage: /agents create project|user NAME");
                };
                let user = match *scope {
                    "project" => false,
                    "user" => true,
                    _ => bail!("Agent location must be project or user"),
                };
                if let Some(error) = agent_definition::validate_agent_type(name) {
                    bail!("{error}");
                }
                print!("Agent description (when to use): ");
                io::stdout().flush()?;
                let mut description = String::new();
                if io::stdin().read_line(&mut description)? == 0 {
                    bail!("Agent creation cancelled: input closed");
                }
                println!("Enter the agent system prompt. End with a line containing .end");
                let mut lines = Vec::new();
                loop {
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line)? == 0 {
                        bail!("Agent creation cancelled: prompt was not completed");
                    }
                    if line.trim_end_matches(['\r', '\n']) == ".end" {
                        break;
                    }
                    lines.push(line.trim_end_matches(['\r', '\n']).to_owned());
                }
                print!("Allowed tools (comma-separated, blank for all): ");
                io::stdout().flush()?;
                let mut tools = String::new();
                if io::stdin().read_line(&mut tools)? == 0 {
                    bail!("Agent creation cancelled: input closed");
                }
                print!("Preferred model (blank to inherit): ");
                io::stdout().flush()?;
                let mut model = String::new();
                if io::stdin().read_line(&mut model)? == 0 {
                    bail!("Agent creation cancelled: input closed");
                }
                let tools = tools
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let definition = agent_definition::AgentDefinition {
                    agent_type: (*name).to_owned(),
                    when_to_use: description.trim().to_owned(),
                    system_prompt: lines.join("\n"),
                    tools: (!tools.is_empty()).then_some(tools),
                    color: None,
                    model: (!model.trim().is_empty()).then(|| model.trim().to_owned()),
                    memory: None,
                    effort: None,
                };
                if !confirm_custom_agent_warnings(&definition, agent.workspace_root(), user)? {
                    return Ok("Agent creation cancelled.".into());
                }
                let path =
                    agent_definition::save_agent(&definition, agent.workspace_root(), user, false)?;
                Ok(format!("Created agent {} at {}", name, path.display()))
            })();
            match outcome {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Agent creation failed: {error:#}"),
            }
            continue;
        }
        if input.starts_with("/agents edit ") {
            let args = input.trim_start_matches("/agents edit ").trim();
            let outcome = (|| -> Result<String> {
                let parts = args.split_whitespace().collect::<Vec<_>>();
                let [scope, name] = parts.as_slice() else {
                    bail!("Usage: /agents edit project|user NAME");
                };
                let user = match *scope {
                    "project" => false,
                    "user" => true,
                    _ => bail!("Agent location must be project or user"),
                };
                let path = agent_definition::new_file_path(agent.workspace_root(), user, name)?;
                let metadata = std::fs::symlink_metadata(&path)
                    .with_context(|| format!("Agent definition missing: {}", path.display()))?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    bail!("Agent definition must be a regular file, not a symbolic link");
                }
                let original = agent_definition::read_agent_file(&path)?;
                println!(
                    "Current agent definition at {}:\n{}",
                    path.display(),
                    original
                );
                println!("Enter replacement description and system prompt; invalid input leaves the existing file unchanged.");
                print!("New description (when to use): ");
                io::stdout().flush()?;
                let mut description = String::new();
                if io::stdin().read_line(&mut description)? == 0 {
                    bail!("Agent edit cancelled: input closed");
                }
                println!("New system prompt (finish with a line containing .end):");
                let mut lines = Vec::new();
                loop {
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line)? == 0 {
                        bail!("Agent edit cancelled: prompt incomplete");
                    }
                    if line.trim_end_matches(['\r', '\n']) == ".end" {
                        break;
                    }
                    lines.push(line.trim_end_matches(['\r', '\n']).to_owned());
                }
                print!("Allowed tools (comma-separated, blank for all): ");
                io::stdout().flush()?;
                let mut tools = String::new();
                if io::stdin().read_line(&mut tools)? == 0 {
                    bail!("Agent edit cancelled: input closed");
                }
                print!("Preferred model (blank to inherit): ");
                io::stdout().flush()?;
                let mut model = String::new();
                if io::stdin().read_line(&mut model)? == 0 {
                    bail!("Agent edit cancelled: input closed");
                }
                let tools = tools
                    .split(',')
                    .map(str::trim)
                    .filter(|tool| !tool.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let (color, memory, effort) = agent_definition::retained_optional_fields(&original);
                let definition = agent_definition::AgentDefinition {
                    agent_type: (*name).to_owned(),
                    when_to_use: description.trim().to_owned(),
                    system_prompt: lines.join("\n"),
                    tools: (!tools.is_empty()).then_some(tools),
                    color,
                    model: (!model.trim().is_empty()).then(|| model.trim().to_owned()),
                    memory,
                    effort,
                };
                if !confirm_custom_agent_warnings(&definition, agent.workspace_root(), user)? {
                    return Ok("Agent edit cancelled; existing file preserved.".into());
                }
                agent_definition::save_agent(&definition, agent.workspace_root(), user, true)?;
                Ok(format!("Updated {} agent {}", scope, name))
            })();
            match outcome {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Agent edit failed: {error:#}"),
            }
            continue;
        }
        if input.starts_with("/agents delete ") {
            let args = input.trim_start_matches("/agents delete ").trim();
            let outcome = (|| -> Result<String> {
                let parts = args.split_whitespace().collect::<Vec<_>>();
                let [scope, name] = parts.as_slice() else {
                    bail!("Usage: /agents delete project|user NAME");
                };
                let user = match *scope {
                    "project" => false,
                    "user" => true,
                    _ => bail!("Agent location must be project or user"),
                };
                let path = agent_definition::new_file_path(agent.workspace_root(), user, name)?;
                match std::fs::symlink_metadata(&path) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        bail!("Refusing to delete redirected agent definition");
                    }
                    Ok(meta) if !meta.is_file() => bail!("Agent definition is not a file"),
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        bail!("Agent definition does not exist in the selected location");
                    }
                    Err(error) => return Err(error.into()),
                }
                print!("Delete {} agent {}? [y/N]: ", scope, name);
                io::stdout().flush()?;
                let mut answer = String::new();
                if io::stdin().read_line(&mut answer)? == 0 {
                    return Ok("Agent deletion cancelled.".into());
                }
                if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    return Ok("Agent deletion cancelled.".into());
                }
                agent_definition::delete_agent(agent.workspace_root(), user, name)?;
                Ok(format!("Deleted {} agent {}", scope, name))
            })();
            match outcome {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Agent deletion failed: {error:#}"),
            }
            continue;
        }
        if input == "/copy" || input.starts_with("/copy ") {
            let selection = input.strip_prefix("/copy").unwrap_or("").trim();
            // Original copy.tsx presents full-response/code-block choices
            // whenever the selected answer contains Markdown code.
            let outcome = (|| -> Result<Option<String>> {
                if selection == "--reset" {
                    copy_response::save_copy_full_preference(false)?;
                    return Ok(Some("Copy picker restored for future responses.".into()));
                }
                let (age, explicit_code) = copy_response::parse_selection(selection)?;
                let always_full = copy_response::copy_full_preference()?;
                if explicit_code.is_some() || always_full {
                    return agent.copy_response(selection).map(Some);
                }
                let choices = agent.copy_code_block_choices(age)?;
                if choices.is_empty() {
                    return agent.copy_response(selection).map(Some);
                }
                println!("Copy response #{age}:");
                println!("  0) Full response");
                for (index, (language, preview)) in choices.iter().enumerate() {
                    println!("  {}) [{}] {}", index + 1, language, preview);
                }
                println!("  a) Always copy full responses (saved preference)");
                println!("  q) Cancel");
                loop {
                    print!("Select copy content: ");
                    io::stdout().flush()?;
                    let mut choice = String::new();
                    if io::stdin().read_line(&mut choice)? == 0 {
                        return Ok(None);
                    }
                    let choice = choice.trim();
                    if choice.eq_ignore_ascii_case("q") {
                        return Ok(None);
                    }
                    if choice.eq_ignore_ascii_case("a") {
                        copy_response::save_copy_full_preference(true)?;
                        return agent.copy_response(&age.to_string()).map(Some);
                    }
                    if choice == "0" {
                        return agent.copy_response(&age.to_string()).map(Some);
                    }
                    if let Ok(index) = choice.parse::<usize>() {
                        if index > 0 && index <= choices.len() {
                            return agent
                                .copy_response(&format!("{age} --code {index}"))
                                .map(Some);
                        }
                    }
                    eprintln!("Choose 0-{}, a, or q.", choices.len());
                }
            })();
            match outcome {
                Ok(Some(message)) => println!("{message}"),
                Ok(None) => println!("Copy cancelled."),
                Err(error) => eprintln!("Copy failed: {error:#}"),
            }
            continue;
        }
        if input == "/branch" || input.starts_with("/branch ") {
            let name = input.strip_prefix("/branch").unwrap_or("").trim();
            match agent.branch_session((!name.is_empty()).then_some(name)) {
                Ok(message) => {
                    prompt_history = prompt_history::PromptHistory::new(
                        agent.workspace_root(),
                        &agent.session_id(),
                    )?;
                    println!("{message}");
                }
                Err(error) => eprintln!("Cannot branch: {error:#}"),
            }
            continue;
        }
        if let Some(query) = input
            .strip_prefix("/resume")
            .or_else(|| input.strip_prefix("/continue"))
            .filter(|tail| tail.is_empty() || tail.starts_with(' '))
        {
            let query = query.trim();
            let selection = (|| -> Result<Option<String>> {
                if !query.is_empty() {
                    return Ok(Some(query.to_owned()));
                }
                // Original /resume with no arguments opens a session picker.
                // Exclude the current conversation and team sidechains.
                let sessions = agent.resumable_sessions()?;
                if sessions.is_empty() {
                    println!("No conversations found to resume.");
                    return Ok(None);
                }
                println!("Choose a conversation to resume:");
                for (index, session) in sessions.iter().take(20).enumerate() {
                    let branch = session.git_branch.as_deref().unwrap_or("");
                    println!(
                        "  {}) {} [{}] {} messages",
                        index + 1,
                        session.display_title(),
                        branch,
                        session.messages
                    );
                }
                print!("Session number (q to cancel): ");
                io::stdout().flush()?;
                let mut choice = String::new();
                if io::stdin().read_line(&mut choice)? == 0
                    || choice.trim().eq_ignore_ascii_case("q")
                {
                    return Ok(None);
                }
                let index = choice
                    .trim()
                    .parse::<usize>()
                    .context("Enter a numeric session selection")?;
                if index == 0 || index > sessions.len().min(20) {
                    bail!("Session selection is out of range");
                }
                Ok(Some(sessions[index - 1].id.to_string()))
            })();
            match selection {
                Ok(Some(selected)) => match agent.resume_interactive_session(&selected).await {
                    Ok(message) => {
                        prompt_history = prompt_history::PromptHistory::new(
                            agent.workspace_root(),
                            &agent.session_id(),
                        )?;
                        println!("{message}");
                    }
                    Err(error) => eprintln!("Resume failed: {error:#}"),
                },
                Ok(None) => println!("Resume cancelled."),
                Err(error) => eprintln!("Resume selection failed: {error:#}"),
            }
            continue;
        }
        if input == "/rename" || input.starts_with("/rename ") {
            match agent
                .rename_session_or_generate(input.trim_start_matches("/rename").trim())
                .await
            {
                Ok(result) => println!("{result}"),
                Err(err) => eprintln!("{err:#}"),
            }
            continue;
        }
        if input == "/export" || input.starts_with("/export ") {
            let value = input.trim_start_matches("/export").trim();
            let outcome = (|| -> Result<Option<std::path::PathBuf>> {
                if !value.is_empty() {
                    return agent.export_session(Some(value)).map(Some);
                }
                // Match ExportDialog: preview and confirm a filename before
                // creating the export; the user can rename or cancel.
                let (suggested, preview) = agent.export_dialog_preview();
                println!("Export conversation preview:\n{preview}");
                if preview.chars().count() >= 1200 {
                    println!("... (preview truncated; full conversation will be exported)");
                }
                print!("Export filename [{suggested}] (q to cancel): ");
                io::stdout().flush()?;
                let mut chosen = String::new();
                if io::stdin().read_line(&mut chosen)? == 0 {
                    return Ok(None);
                }
                let chosen = chosen.trim();
                if chosen.eq_ignore_ascii_case("q") {
                    return Ok(None);
                }
                let filename = if chosen.is_empty() {
                    suggested.as_str()
                } else {
                    chosen
                };
                agent.export_session(Some(filename)).map(Some)
            })();
            match outcome {
                Ok(Some(path)) => println!("Conversation exported to: {}", path.display()),
                Ok(None) => println!("Export cancelled."),
                Err(err) => eprintln!("Failed to export conversation: {err:#}"),
            }
            continue;
        }
        if let Some(arg) = input.strip_prefix("/env ") {
            if let Some(name) = arg.strip_prefix("-u ") {
                agent.remove_environment_variable(name.trim());
                println!("Unset session environment: {}", name.trim());
            } else if let Some((name, value)) = arg.split_once('=') {
                agent.set_environment_variable(name.trim(), value)?;
                println!("Set session environment: {}", name.trim());
            } else {
                eprintln!("Usage: /env NAME=VALUE or /env -u NAME");
            }
            continue;
        }
        if input == "/add-dir" || input.starts_with("/add-dir ") {
            let mut entered = String::new();
            let mut raw = input.trim_start_matches("/add-dir").trim();
            if raw.is_empty() {
                print!("Directory path: ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut entered)?;
                raw = entered.trim();
            }
            let remember = raw.ends_with(" --remember");
            if remember {
                raw = raw.trim_end_matches(" --remember").trim_end();
            }
            match agent.validate_workspace_directory(raw)? {
                workspace_dirs::Validation::EmptyPath => {
                    eprintln!("Usage: /add-dir PATH [--remember]")
                }
                workspace_dirs::Validation::PathNotFound(p) => {
                    eprintln!("Path {} was not found", p.display())
                }
                workspace_dirs::Validation::NotADirectory(p) => eprintln!(
                    "{} is not a directory. Try {}",
                    p.display(),
                    p.parent().unwrap_or(&p).display()
                ),
                workspace_dirs::Validation::AlreadyAccessible {
                    path,
                    existing_root,
                } => println!(
                    "{} is already accessible via {}",
                    path.display(),
                    existing_root.display()
                ),
                workspace_dirs::Validation::Success(path) => {
                    print!(
                        "Allow {}? [y] this session / [r] remember / [N] cancel: ",
                        path.display()
                    );
                    io::stdout().flush()?;
                    let mut response = String::new();
                    io::stdin().read_line(&mut response)?;
                    let decision = response.trim().to_ascii_lowercase();
                    if matches!(decision.as_str(), "y" | "yes" | "r" | "remember") {
                        agent.add_workspace_directory(path.clone());
                        if remember || matches!(decision.as_str(), "r" | "remember") {
                            let result = permission_settings::SettingsPaths::for_workspace(
                                agent.workspace_root(),
                            )
                            .and_then(|paths| {
                                paths.add_directory(
                                    permission_settings::EditableSource::Local,
                                    &path,
                                )
                            });
                            match result {
                                Ok(true) => {
                                    println!("Added {} and saved to local settings", path.display())
                                }
                                Ok(false) => eprintln!(
                                    "Added {} for this session; managed policy blocks saving",
                                    path.display()
                                ),
                                Err(err) => eprintln!(
                                    "Added {} for this session; settings save failed: {err:#}",
                                    path.display()
                                ),
                            }
                        } else {
                            println!("Added {} for this session", path.display());
                        }
                    } else {
                        println!("Did not add {}", path.display());
                    }
                }
            }
            continue;
        }
        if let Some(parsed) = slash_command::parse_slash_command(input)
            .filter(|parsed| parsed.command_name == "model" && !parsed.is_mcp)
        {
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
        if let Some(rest) = input.strip_prefix("/trigger ") {
            let mut parts = rest.trim().splitn(3, char::is_whitespace);
            let action = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("").trim();
            let tail = parts.next().unwrap_or("").trim();
            match action {
                "list" if name.is_empty() => match local_triggers::list(agent.workspace_root()) {
                    Ok(names) => {
                        for name in names {
                            println!("{name}");
                        }
                    }
                    Err(error) => eprintln!("Cannot list triggers: {error:#}"),
                },
                "set" => match local_triggers::set(agent.workspace_root(), name, tail) {
                    Ok(()) => println!("Saved local trigger: {name}"),
                    Err(error) => eprintln!("Cannot save trigger: {error:#}"),
                },
                "create" => match local_triggers::create(agent.workspace_root(), name, tail) {
                    Ok(()) => println!("Created local trigger: {name}"),
                    Err(error) => eprintln!("Cannot create trigger: {error:#}"),
                },
                "update" => match local_triggers::change(agent.workspace_root(), name, tail) {
                    Ok(()) => println!("Updated local trigger: {name}"),
                    Err(error) => eprintln!("Cannot update trigger: {error:#}"),
                },
                "get" => match local_triggers::get(agent.workspace_root(), name) {
                    Ok(prompt) => println!("{prompt}"),
                    Err(error) => eprintln!("Cannot read trigger: {error:#}"),
                },
                "delete" => match local_triggers::delete(agent.workspace_root(), name) {
                    Ok(()) => println!("Deleted local trigger: {name}"),
                    Err(error) => eprintln!("Cannot delete trigger: {error:#}"),
                },
                "run" => match local_triggers::get(agent.workspace_root(), name) {
                    Ok(prompt) => match agent.ask(&prompt).await {
                        Ok(answer) => println!("{answer}"),
                        Err(error) => eprintln!("Trigger failed: {error:#}"),
                    },
                    Err(error) => eprintln!("Cannot run trigger: {error:#}"),
                },
                _ => eprintln!("Usage: /trigger create NAME GOAL | get NAME | update NAME GOAL | run NAME | list | delete NAME"),
            }
            continue;
        }
        if let Some((name, args)) = input
            .strip_prefix('/')
            .map(|t| t.split_once(' ').unwrap_or((t, "")))
        {
            let action = match name {
                "review" => Some(git_workflows::Action::Review),
                "commit" => Some(git_workflows::Action::Commit),
                "commit-push-pr" => Some(git_workflows::Action::CommitPushPr),
                _ => None,
            };
            if name == "ultrareview" || name == "ultraplan" {
                let prepared = if name == "ultrareview" {
                    local_advanced_workflows::review_request(agent.workspace_root(), args)
                } else {
                    local_advanced_workflows::plan_prompt(agent.workspace_root(), args)
                };
                match prepared {
                    Ok(prompt) => match agent.ask_in_plan_mode(&prompt).await {
                        Ok(answer) => println!("{answer}"),
                        Err(error) => eprintln!("Error: {error:#}"),
                    },
                    Err(error) => eprintln!("Cannot prepare /{name}: {error:#}"),
                }
                continue;
            }
            if let Some(action) = action {
                match git_workflows::prompt(action, agent.workspace_root(), args) {
                    Ok(prompt) => match agent.ask(&prompt).await {
                        Ok(answer) => println!("{answer}"),
                        Err(error) => eprintln!("Error: {error:#}"),
                    },
                    Err(error) => eprintln!("Cannot prepare /{name}: {error:#}"),
                }
                continue;
            }
        }
        // /btw runs a one-turn, no-tools fork. It does not append a user
        // message or model response to the active conversation history.
        let mut side_parts = input.trim_start().splitn(2, char::is_whitespace);
        if side_parts
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("/btw"))
        {
            let question = side_parts.next().unwrap_or("");
            let result = agent
                .side_question_request(question)
                .and_then(|request| Ok((agent.profile().clone(), request)));
            match result {
                Ok((profile, request)) => match side_question::answer(profile, request, None).await
                {
                    Ok(reply) => println!(
                        "{}",
                        reply["response"].as_str().unwrap_or("No response received")
                    ),
                    Err(error) => eprintln!("Side question failed: {error:#}"),
                },
                Err(error) => eprintln!("Side question failed: {error:#}"),
            }
            continue;
        }
        // Both plugin and local skills share the SDK's real command
        // expansion and user-invocable permission checks.
        if let Some(parsed) = slash_command::parse_slash_command(input) {
            match expand_user_skill_prompt(
                agent,
                config,
                &parsed.command_name,
                &parsed.args,
                parsed.is_mcp,
            ) {
                Ok(Some(prompt)) => {
                    onboarding.maybe_mark_complete()?;
                    match agent.ask(&prompt).await {
                        Ok(answer) => println!("{answer}"),
                        Err(error) => eprintln!("Skill command failed: {error:#}"),
                    }
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("Cannot invoke skill command: {error:#}");
                    continue;
                }
            }
        }
        // The original 2.1.88 keyword.ts recognizes standalone launch
        // directives in ordinary text, but ignores quoted references, paths,
        // switches, questions and slash commands. Respect the same boundaries
        // for the provider-neutral local advanced workflows.
        if let Some(goal) = local_advanced_keywords::replace_ultraplan_keyword(input) {
            if goal.trim().is_empty() {
                println!("Usage: /ultraplan GOAL");
                continue;
            }
            match local_advanced_workflows::plan_prompt(agent.workspace_root(), &goal) {
                Ok(prompt) => {
                    onboarding.maybe_mark_complete()?;
                    match agent.ask_in_plan_mode(&prompt).await {
                        Ok(answer) => println!("{answer}"),
                        Err(error) => eprintln!("Local UltraPlan failed: {error:#}"),
                    }
                }
                Err(error) => eprintln!("Cannot prepare UltraPlan: {error:#}"),
            }
            continue;
        }
        // keyword.ts makes ultrareview a visual suggestion only. The user
        // must explicitly invoke /ultrareview; ordinary requests stay ordinary.
        // The original marks completion on prompt submission, not on a
        // successful model response; local project instructions may have
        // been created by the user outside this chat.
        onboarding.maybe_mark_complete()?;
        match agent.ask(input).await {
            Ok(answer) => println!("{answer}"),
            Err(err) => eprintln!("Error: {err:#}"),
        }
    }
    Ok(())
}

/// Run a SessionStart-provided initial user turn after SDK initialization
/// has installed the caller's prompt and runtime options (print.ts:2914-2917).
/// Each SDK model turn can be interrupted while the input reader continues
/// accepting requests. Dropping the Agent future stops the active provider
/// request and propagates cancellation into any awaited tool or hook future.
/// Replace the last SDK text block without dropping other text or images.
fn sdk_replace_last_text(input: &mut multimodal::Input, replacement: String) {
    if let Some(blocks) = input.blocks.as_mut() {
        if let Some(last_text) = blocks
            .iter_mut()
            .rev()
            .find(|block| block["type"] == "text")
        {
            last_text["text"] = json!(replacement);
        }
        input.preview = blocks
            .iter()
            .map(|block| match block["type"].as_str() {
                Some("text") => block["text"].as_str().unwrap_or("").to_owned(),
                Some("image") => format!(
                    "[image: {}]",
                    block["mime_type"].as_str().unwrap_or("unknown")
                ),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n");
    } else {
        input.preview = replacement;
    }
}

async fn sdk_interruptible_turn(
    agent: &mut Agent,
    prompt: &str,
    blocks: Option<Vec<Value>>,
    message_id: Option<uuid::Uuid>,
    interrupts: &tokio::sync::watch::Receiver<u64>,
    input_epoch: u64,
    plan_mode: bool,
) -> Result<String> {
    let mut observer = interrupts.clone();
    // The stdin reader may have parsed a later interrupt while the main loop
    // was preparing an earlier user event. Match event order, not scheduling.
    if *observer.borrow_and_update() > input_epoch {
        bail!("SDK request interrupted");
    }
    tokio::select! {
        biased;
        changed = observer.changed() => {
            if changed.is_ok() {
                bail!("SDK request interrupted");
            }
            bail!("SDK interrupt control channel closed");
        }
        response = async {
            if plan_mode {
                agent.ask_with_blocks_in_plan_mode(prompt, blocks, message_id).await
            } else {
                agent.ask_with_blocks_for_message(prompt, blocks, message_id).await
            }
        } => response,
    }
}

async fn sdk_run_initial_turn(
    agent: &mut Agent,
    interrupts: &tokio::sync::watch::Receiver<u64>,
    input_epoch: u64,
) {
    match sdk_interruptible_turn(agent, "", None, None, interrupts, input_epoch, false).await {
        Ok(answer) => print_json!(json!({
            "type":"result","session_id":agent.session_id(),
            "content":answer,"usage":agent.usage(),"cost":agent.cost_ledger()
        })),
        Err(error) => print_json!(json!({
            "type":"error","session_id":agent.session_id(),
            "message":format!("{error:#}")
        })),
    }
}

/// An independent SDK title result is committed only by the main Agent
/// actor, keeping session state and per-model usage as single-writer data.
struct SdkTitleDone {
    id: String,
    session_id: uuid::Uuid,
    profile: config::ProviderProfile,
    persist: bool,
    elapsed: std::time::Duration,
    outcome: Result<(Option<String>, Option<protocol::TokenUsage>)>,
}

fn sdk_deliver_title(agent: &mut Agent, done: SdkTitleDone) {
    let result = match done.outcome {
        Ok((title, usage)) => agent.finish_sdk_title(
            done.session_id,
            &done.profile,
            usage,
            title,
            done.persist,
            done.elapsed,
        ),
        Err(error) => {
            // The source returns null on title-model failure while still
            // charging the request duration to the original session.
            let accounting = agent.finish_sdk_title(
                done.session_id,
                &done.profile,
                None,
                None,
                false,
                done.elapsed,
            );
            if let Err(accounting_error) = accounting {
                Err(accounting_error)
            } else {
                eprintln!("Session title generation failed: {error:#}");
                Ok(None)
            }
        }
    };
    match result {
        Ok(title) => print_json!(json!({"type":"control_response","response":{
            "subtype":"success","request_id":done.id,"response":{"title":title}
        }})),
        Err(error) => print_json!(json!({"type":"control_response","response":{
            "subtype":"error","request_id":done.id,"error":format!("{error:#}")
        }})),
    }
}

async fn sdk(agent: &mut Agent, config: &Config) -> Result<()> {
    let mut queue = message_queue::Queue::default();
    queue.enable_journal(agent.workspace_root(), &agent.session_id())?;
    let mut query_guard = query_guard::QueryGuard::default();
    let mut sdk_initialized = false;
    let mut sdk_end_requested = false;
    let mut side_question_tasks = Vec::<tokio::task::JoinHandle<()>>::new();
    let (title_tx, mut title_rx) = tokio::sync::mpsc::unbounded_channel::<SdkTitleDone>();
    let mut title_pending = 0usize;
    print_json!(json!({"type":"system","subtype":"init",
        "session_id":agent.session_id(),"provider":agent.profile().id,
        "model":agent.profile().model,
        "usage_schema":runtime_contracts::empty_usage()}));
    // Source print.ts queues the SessionStart message and drains it only
    // after initialize has installed systemPrompt, agents and hooks. Eager
    // execution previously rejected a subsequent valid initialize request.
    let mut initial_turn_pending = agent.has_initial_user_message();
    // print.ts::structuredInput runs independently of the query consumer.
    // A dedicated blocking reader can signal an interrupt while the Agent
    // awaits a model, tool, memory or hook request. Other controls remain in
    // FIFO order on the main SDK actor, preserving its unique state owner.
    let (input_tx, mut input_rx) =
        tokio::sync::mpsc::unbounded_channel::<io::Result<(String, u64)>>();
    let (interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(0u64);
    let _interrupt_keepalive = interrupt_tx.clone();
    std::thread::spawn(move || {
        let mut input_epoch = 0u64;
        for input in io::stdin().lock().lines() {
            let event = input.map(|line| {
                let is_interrupt = serde_json::from_str::<Value>(&line)
                    .ok()
                    .is_some_and(|value| {
                        let kind = value.get("type").and_then(Value::as_str);
                        // Original print.ts aborts on explicit interrupt,
                        // end_session and a higher-priority queued message.
                        (kind == Some("control_request")
                            && matches!(
                                value.pointer("/request/subtype").and_then(Value::as_str),
                                Some("interrupt" | "end_session")
                            ))
                            || (matches!(kind, Some("control" | "control_request"))
                                && matches!(
                                    value.get("action").and_then(Value::as_str),
                                    Some("interrupt" | "end_session")
                                ))
                            || (kind == Some("user")
                                && value.get("queued").and_then(Value::as_bool) == Some(true)
                                && value.get("priority").and_then(Value::as_str) == Some("now"))
                    });
                if is_interrupt {
                    input_epoch = input_epoch.wrapping_add(1);
                    interrupt_tx.send_modify(|revision| *revision = input_epoch);
                }
                (line, input_epoch)
            });
            if input_tx.send(event).is_err() {
                break;
            }
        }
    });
    loop {
        // Completed title requests can publish their replies while stdin is
        // idle; they never wait behind the next SDK control request.
        let incoming = tokio::select! {
            biased;
            Some(done) = title_rx.recv(), if title_pending > 0 => {
                title_pending -= 1;
                sdk_deliver_title(agent, done);
                continue;
            }
            input = input_rx.recv() => input,
            // A hook can finish after its originating model turn. Wake the
            // SDK event loop even without new stdin, just like the source
            // AsyncHookRegistry response-delivery path.
            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                agent.pump_async_hook_events().await?;
                // The 2.1.88 asyncRewake path injects a task notification
                // between API rounds, or wakes the model when the SDK is idle.
                if sdk_initialized && !initial_turn_pending && !sdk_end_requested {
                    let notifications = agent.take_rewake_notifications();
                    if !notifications.is_empty() {
                        let prompt = notifications
                            .iter()
                            .map(|message| agent::wrap_in_system_reminder(message))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let input_epoch = *interrupt_rx.borrow();
                        match sdk_interruptible_turn(
                            agent, &prompt, None, None, &interrupt_rx, input_epoch, false,
                        ).await {
                            Ok(answer) => print_json!(json!({
                                "type":"result","session_id":agent.session_id(),
                                "content":answer,"usage":agent.usage(),
                                "cost":agent.cost_ledger()
                            })),
                            Err(error) => print_json!(json!({
                                "type":"error","session_id":agent.session_id(),
                                "message":format!("{error:#}")
                            })),
                        }
                    }
                }
                continue;
            }
        };
        let Some(input) = incoming else { break };
        let (line, input_epoch) = input?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                print_json!(json!({"type":"error","message":error.to_string()}));
                continue;
            }
        };
        let event = control_message_compat::normalize_control_message_keys(event);
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("user");
        match kind {
            "user" => {
                // A stream without explicit initialize still executes the
                // SessionStart turn first, preserving user message ordering.
                if initial_turn_pending {
                    initial_turn_pending = false;
                    sdk_run_initial_turn(agent, &interrupt_rx, input_epoch).await;
                }
                // Accept both direct content and SDK message envelopes.
                // Older SDK clients send message.content with replay metadata.
                let raw_content = event
                    .get("content")
                    .or_else(|| event.pointer("/message/content"));
                let Some(raw_content) = raw_content else {
                    print_json!(json!({"type":"error","message":"User event requires content"}));
                    continue;
                };
                let content = match agent.attach_local_files(
                    raw_content,
                    event
                        .get("file_attachments")
                        .or_else(|| event.get("fileAttachments")),
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        print_json!(json!({"type":"error","message":format!("{error:#}")}));
                        continue;
                    }
                };
                let prompt_value = &content;
                // First SDK user message implicitly initializes the stream,
                // matching the original print.ts user-message path.
                sdk_initialized = true;
                if event.get("queued").and_then(Value::as_bool) == Some(true) {
                    let mut command: message_queue::Command = match serde_json::from_value(json!({
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
                        Ok(command) => command,
                        Err(error) => {
                            print_json!(json!({"type":"error",
                                "message":format!("Invalid queued user command: {error}")}));
                            continue;
                        }
                    };
                    command.agent_id = event
                        .get("agentId")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let identity = queue.enqueue(command, false);
                    print_json!(json!({"type":"queued","identity":identity,
                        "revision":queue.revision(),"length":queue.len()}));
                    continue;
                }
                // Preserve and validate SDK text/image blocks before routing
                // to a provider. Never silently flatten away image payloads.
                let mut user = match multimodal::parse(prompt_value) {
                    Ok(user) => user,
                    Err(error) => {
                        print_json!(json!({"type":"error",
                            "message":format!("Unsupported user content: {error:#}")}));
                        continue;
                    }
                };
                // The original SDK user input and REPL both recognize advanced
                // commands. Keep images, earlier blocks, UUID and skipSlashCommands
                // intact, and apply plan permissions only for this one turn.
                let mut advanced_plan_turn = false;
                if event.get("skipSlashCommands").and_then(Value::as_bool) != Some(true) {
                    // processUserInput.ts:336-344 only interprets the LAST
                    // input block when it is text. A trailing image must not
                    // accidentally invoke a command in an earlier block.
                    let candidate = prompt_value.as_str().or_else(|| {
                        let last = prompt_value.as_array()?.last()?;
                        (last.get("type").and_then(Value::as_str) == Some("text"))
                            .then(|| last.get("text").and_then(Value::as_str))
                            .flatten()
                    });
                    if let Some(candidate) = candidate {
                        match local_advanced_workflows::sdk_user_prompt(
                            agent.workspace_root(),
                            candidate,
                        ) {
                            Ok(Some(expanded)) => {
                                sdk_replace_last_text(&mut user, expanded);
                                advanced_plan_turn = true;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                print_json!(json!({"type":"error",
                                    "message":format!("Cannot prepare SDK advanced workflow: {error:#}")}));
                                continue;
                            }
                        }
                        if !advanced_plan_turn {
                            if let Some(parsed) = slash_command::parse_slash_command(candidate) {
                                match expand_user_skill_prompt(
                                    agent,
                                    config,
                                    &parsed.command_name,
                                    &parsed.args,
                                    parsed.is_mcp,
                                ) {
                                    Ok(Some(expanded)) => {
                                        sdk_replace_last_text(&mut user, expanded)
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        print_json!(json!({"type":"error",
                                            "message":format!("Cannot invoke skill command: {error:#}")}));
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
                // QueryEngine normally emits this replay during ask().
                // The Rust SDK entrypoint owns the delivery acknowledgement.
                if event
                    .get("replay_user_messages")
                    .or_else(|| event.get("replayUserMessages"))
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    if let Some(uuid) = event.get("uuid").and_then(Value::as_str) {
                        sdk_queue::emit_user_replay(prompt_value, uuid, &agent.session_id());
                    }
                }
                // The same UUID identifies both the SDK user message and
                // every file edit performed by its agent/tool execution.
                let message_id = event
                    .get("uuid")
                    .and_then(Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .map(|id| sdk_message_id(id, &agent.session_id()));
                match sdk_interruptible_turn(
                    agent,
                    &user.preview,
                    user.blocks,
                    message_id,
                    &interrupt_rx,
                    input_epoch,
                    advanced_plan_turn,
                )
                .await
                {
                    Ok(answer) => print_json!(json!({
                        "type":"result","session_id":agent.session_id(),
                        "content":answer,"usage":agent.usage(),"cost":agent.cost_ledger()
                    })),
                    Err(error) => print_json!(json!({
                        "type":"error","session_id":agent.session_id(),
                        "message":format!("{error:#}")
                    })),
                }
            }
            // SDK transport heartbeats are deliberately silent; they must
            // not create errors or mutate the active conversation.
            "keep_alive" => {}
            "control_request" if event.get("request").is_some() => {
                // Source cli/print.ts:3783-3814 executes title generation
                // independently of the SDK control message reader.
                if event.pointer("/request/subtype").and_then(Value::as_str)
                    == Some("generate_session_title")
                {
                    let prepared: Result<(String, bool, Option<protocol::ModelRequest>)> = (|| {
                        let id = event
                            .get("request_id")
                            .and_then(Value::as_str)
                            .filter(|id| !id.is_empty())
                            .ok_or_else(|| {
                                anyhow::anyhow!("generate_session_title requires request_id")
                            })?;
                        let description = event
                            .pointer("/request/description")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "generate_session_title requires description string"
                                )
                            })?;
                        let persist = match event.pointer("/request/persist") {
                            None => false,
                            Some(Value::Bool(value)) => *value,
                            _ => bail!("generate_session_title persist must be a boolean"),
                        };
                        Ok((id.to_owned(), persist, agent.sdk_title_request(description)))
                    })(
                    );
                    match prepared {
                        Ok((id, persist, Some(request))) => {
                            let profile = agent.profile().clone();
                            let session_id = uuid::Uuid::parse_str(&agent.session_id())?;
                            let sender = title_tx.clone();
                            title_pending = title_pending.saturating_add(1);
                            tokio::spawn(async move {
                                let started = std::time::Instant::now();
                                let outcome: Result<(
                                    Option<String>,
                                    Option<protocol::TokenUsage>,
                                )> = async {
                                    let client = provider::create(&profile)?;
                                    let completion = client.complete(&request).await?;
                                    let usage = completion.usage;
                                    if completion
                                        .message
                                        .tool_calls
                                        .as_ref()
                                        .is_some_and(|calls| !calls.is_empty())
                                    {
                                        return Ok((None, usage));
                                    }
                                    let title = serde_json::from_str::<Value>(
                                        &completion.message.visible_text(),
                                    )
                                    .ok()
                                    .and_then(|value| {
                                        value
                                            .get("title")
                                            .and_then(Value::as_str)
                                            .map(str::to_owned)
                                    })
                                    .map(|value| value.trim().to_owned())
                                    .filter(|value| !value.is_empty());
                                    Ok((title, usage))
                                }
                                .await;
                                let _ = sender.send(SdkTitleDone {
                                    id,
                                    session_id,
                                    profile,
                                    persist,
                                    elapsed: started.elapsed(),
                                    outcome,
                                });
                            });
                        }
                        Ok((id, _, None)) => print_json!(json!({
                            "type":"control_response","response":{
                                "subtype":"success","request_id":id,
                                "response":{"title":null}
                            }
                        })),
                        Err(error) => print_json!(json!({
                            "type":"control_response","response":{
                                "subtype":"error",
                                "request_id":event.get("request_id")
                                    .cloned().unwrap_or(Value::Null),
                                "error":format!("{error:#}")
                            }
                        })),
                    }
                    continue;
                }
                // side_question is fire-and-forget in the original SDK. Its
                // model request must not hold up subsequent stdin controls.
                if event.pointer("/request/subtype").and_then(Value::as_str)
                    == Some("side_question")
                {
                    let prepared: Result<(
                        String,
                        config::ProviderProfile,
                        protocol::ModelRequest,
                        Option<u32>,
                    )> = (|| {
                        let id = event
                            .get("request_id")
                            .and_then(Value::as_str)
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| anyhow::anyhow!("side_question requires request_id"))?;
                        let question = event
                            .pointer("/request/question")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                            anyhow::anyhow!("side_question requires question string")
                        })?;
                        Ok((
                            id.to_owned(),
                            agent.profile().clone(),
                            agent.side_question_request(question)?,
                            agent.sdk_thinking_budget(),
                        ))
                    })();
                    match prepared {
                        Ok((id, profile, request, thinking_budget)) => {
                            side_question_tasks.retain(|task| !task.is_finished());
                            side_question_tasks.push(tokio::spawn(async move {
                                let outcome =
                                    side_question::answer(profile, request, thinking_budget).await;
                                let reply = match outcome {
                                    Ok(response) => json!({"type":"control_response",
                                        "response":{"subtype":"success",
                                            "request_id":id,"response":response}}),
                                    Err(error) => json!({"type":"control_response",
                                        "response":{"subtype":"error",
                                            "request_id":id,"error":format!("{error:#}")}}),
                                };
                                print_json!(reply);
                            }));
                        }
                        Err(error) => print_json!(json!({"type":"control_response",
                            "response":{"subtype":"error",
                                "request_id":event.get("request_id")
                                    .cloned().unwrap_or(Value::Null),
                                "error":format!("{error:#}")}})),
                    }
                    continue;
                }
                let is_initialize =
                    event.pointer("/request/subtype").and_then(Value::as_str) == Some("initialize");
                let response = sdk_nested_control(
                    agent,
                    config,
                    &event,
                    &mut queue,
                    &mut sdk_initialized,
                    &mut sdk_end_requested,
                )
                .await;
                let initialized = is_initialize
                    && response
                        .pointer("/response/subtype")
                        .and_then(Value::as_str)
                        == Some("success");
                print_json!(response);
                if initialized && initial_turn_pending {
                    initial_turn_pending = false;
                    sdk_run_initial_turn(agent, &interrupt_rx, input_epoch).await;
                }
                if sdk_end_requested {
                    break;
                }
            }
            "control" | "control_request" => {
                macro_rules! send_control {
                    ($reply:expr) => {{
                        let mut response: Value = $reply;
                        if let Some(id) = event.get("request_id") {
                            response["request_id"] = id.clone();
                        }
                        print_json!(response);
                    }};
                }
                let action = event.get("action").and_then(Value::as_str).unwrap_or("");
                if action == "interrupt" {
                    send_control!(json!({"type":"control_response",
                        "action":"interrupt","ok":true}));
                    continue;
                }
                if action.starts_with("queue_") {
                    let outcome = sdk_queue::control(
                        action,
                        &event,
                        &mut queue,
                        &mut query_guard,
                        agent,
                        config,
                    )
                    .await;
                    match outcome {
                        Ok(result) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":result})),
                        Err(error) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if action == "mcp_prompt_commands" {
                    let outcome = agent.list_mcp_prompt_commands().await;
                    match outcome {
                        Ok(value) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":value})),
                        Err(error) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if matches!(
                    action,
                    "mcp_resources"
                        | "mcp_prompts"
                        | "mcp_resource_templates"
                        | "mcp_read_resource"
                        | "mcp_get_prompt"
                        | "mcp_resource_subscribe"
                        | "mcp_resource_unsubscribe"
                        | "mcp_resource_updates"
                        | "mcp_poll_resource_updates"
                ) {
                    let result = mcp_control_request(agent, action, &event).await;
                    match result {
                        Ok(value) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":value})),
                        Err(error) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    };
                    continue;
                }
                if action == "mailbox_send" {
                    let parsed = event
                        .get("message")
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("mailbox_send requires message"))
                        .and_then(|v| {
                            serde_json::from_value::<in_memory_mailbox::Message>(v)
                                .map_err(anyhow::Error::from)
                        });
                    match parsed {
                        Ok(message) => {
                            let revision = agent.send_transient(message);
                            send_control!(json!({"type":"control_response","action":action,
                                "ok":true,"revision":revision}));
                        }
                        Err(error) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if action == "mailbox_poll" {
                    let message = agent.poll_transient();
                    send_control!(json!({"type":"control_response","action":action,
                        "ok":true,"message":message,"revision":agent.transient_revision()}));
                    continue;
                }
                if action == "context_usage" || action == "get_context_usage" {
                    match agent.context_usage() {
                        Ok(value) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":true,"result":value})),
                        Err(error) => send_control!(json!({"type":"control_response",
                            "action":action,"ok":false,"error":format!("{error:#}")})),
                    }
                    continue;
                }
                if action == "cost" {
                    send_control!(json!({"type":"control_response","action":action,
                        "ok":true,"cost":agent.cost_ledger()}));
                    continue;
                }
                if action == "permission_mode" {
                    send_control!(json!({"type":"control_response","action":action,
                        "ok":true,"mode":agent.current_permission_mode()}));
                    continue;
                }
                if action == "tasks" {
                    match agent.background_tasks() {
                        Ok(tasks) => send_control!(json!({
                            "type":"control_response","action":action,
                            "ok":true,"result":serde_json::from_str::<Value>(&tasks)?
                        })),
                        Err(error) => send_control!(json!({
                            "type":"control_response","action":action,
                            "ok":false,"error":format!("{error:#}")
                        })),
                    }
                    continue;
                }
                let result = match action {
                    "clear" => agent.clear_history().await,
                    "compact" => {
                        agent
                            .compact_history_custom(
                                event
                                    .get("instructions")
                                    .or_else(|| event.get("customInstructions"))
                                    .and_then(Value::as_str)
                                    .unwrap_or(""),
                            )
                            .await
                    }
                    "model" => {
                        let provider = event.get("provider").and_then(Value::as_str);
                        let model = event.get("model").and_then(Value::as_str);
                        config
                            .select(provider, model)
                            .and_then(|p| agent.switch_model(p))
                    }
                    _ => Err(anyhow::anyhow!("Unknown control action: {action}")),
                };
                match result {
                    Ok(()) => send_control!(json!({"type":"control_response",
                        "action":action,"ok":true})),
                    Err(error) => send_control!(json!({"type":"control_response",
                        "action":action,"ok":false,"error":format!("{error:#}")})),
                };
            }
            other => print_json!(json!({"type":"error",
                "message":format!("Unsupported SDK event {other}")})),
        }
    }
    // Cancel orphaned asynchronous shell hooks on SDK stream shutdown.
    // The SessionEnd event will also finalize in the normal Agent path.
    for event in hooks::finalize_async_hooks(&agent.session_id()).await {
        print_json!(event);
    }
    // Bound the post-EOF flush window for accepted background title
    // requests; persist replies through the main Agent before exiting.
    let title_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while title_pending > 0 {
        let remaining = title_deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, title_rx.recv()).await {
            Ok(Some(done)) => {
                title_pending -= 1;
                sdk_deliver_title(agent, done);
            }
            _ => break,
        }
    }
    // A producer may close stdin immediately after issuing a side question.
    // Give already accepted independent replies a bounded chance to flush
    // before the SDK process exits.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    for task in side_question_tasks {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = tokio::time::timeout(remaining, task).await;
    }
    Ok(())
}

fn sdk_visible_commands(agent: &Agent, _config: &Config) -> Result<Vec<Value>> {
    let implemented = [
        "agents",
        "clear",
        "compact",
        "context",
        "copy",
        "cost",
        "diff",
        "doctor",
        "files",
        "hooks",
        "mcp",
        "permissions",
        "release-notes",
        "skills",
        "stats",
        "status",
        "tasks",
        "usage",
        "tag",
    ];
    let mut commands = command_manifest::visible()
        .filter(|command| implemented.contains(&command.name))
        .map(|command| {
            json!({
                "name":command.name,
                "description":command.description,
                "argumentHint":command.hint.unwrap_or("")
            })
        })
        .collect::<Vec<_>>();
    // The SDK must advertise the same actual user-invocable
    // plugin commands and project skills that its text input can
    // execute. Keep model-only skills out of the public catalog.
    let mut known = commands
        .iter()
        .filter_map(|command| command.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<std::collections::HashSet<_>>();
    for command in plugins::command_descriptors(agent.workspace_root(), agent.enabled_plugins())? {
        let name = command
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Plugin command descriptor missing name"))?;
        if known.insert(name.to_owned()) {
            commands.push(command);
        }
    }
    // list_user_skills already applies the real user-invocable
    // flag and Read permissions. Plugin skills are described
    // above using their manifest-resolved paths.
    for name in agent.list_user_skills()?.lines() {
        if name.contains(':') || !known.insert(name.to_owned()) {
            continue;
        }
        let skill = agent
            .workspace_root()
            .join(".atlas/skills")
            .join(name)
            .join("SKILL.md")
            .canonicalize()?;
        if !skill.starts_with(agent.workspace_root()) {
            bail!("Skill command source leaves authorized workspace")
        }
        let source = std::fs::read_to_string(&skill)?;
        let (description, hint) = skill_arguments::command_metadata(&source, "Project skill");
        commands.push(json!({"name":name,"description":description,
            "argumentHint":hint}));
    }
    Ok(commands)
}

/// Standard SDK request envelope; the legacy flat local command protocol
/// remains available through "control" messages.
async fn sdk_nested_control(
    agent: &mut Agent,
    config: &Config,
    event: &Value,
    queue: &mut message_queue::Queue,
    sdk_initialized: &mut bool,
    sdk_end_requested: &mut bool,
) -> Value {
    let id = event
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let outcome: Result<Value> = async {
        if id.is_empty() {
            bail!("control_request requires request_id")
        }
        let request = event
            .get("request")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("control_request requires request object"))?;
        let subtype = request
            .get("subtype")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("control request requires subtype"))?;
        match subtype {
            "interrupt" => Ok(json!({})),
            "end_session" => {
                *sdk_end_requested = true;
                Ok(json!({}))
            }
            "initialize" => {
                if *sdk_initialized {
                    bail!("SDK initialization already completed for this stream");
                }
                // The Rust session is already attached to locally configured
                // hooks/MCP before stdin is read. Never silently claim to have
                // installed SDK callback hooks or SDK-owned MCP transports.
                for key in [
                    "hooks",
                    "sdkMcpServers",
                    "promptSuggestions",
                    "agentProgressSummaries",
                ] {
                    if let Some(value) = request.get(key) {
                        if !value.is_null()
                            && value != &json!({})
                            && value != &json!([])
                            && value != &json!(false)
                        {
                            bail!("initialize {key} is not supported by this local SDK")
                        }
                    }
                }
                let models = config
                    .providers
                    .iter()
                    .map(|profile| {
                        json!({
                            "value":profile.model,"displayName":format!("{} / {}",
                                profile.id,profile.model),
                            "description":format!("{} provider",profile.kind)
                        })
                    })
                    .collect::<Vec<_>>();
                // Expose only commands actually wired into this Rust chat
                // runtime. The source registration index also contains
                // unported commands, which must not be advertised as usable.
                let commands = sdk_visible_commands(agent, config)?;
                // Check the on-disk agent catalog before changing SDK state.
                agent.sdk_agent_descriptors()?;
                // cli/print.ts::handleInitializeRequest applies both fields
                // before QueryEngine assembles the next model request. Validate
                // before mutating the active Agent so bad init leaves it intact.
                let system_prompt = request
                    .get("systemPrompt")
                    .map(|value| {
                        value.as_str().ok_or_else(|| {
                            anyhow::anyhow!("initialize systemPrompt must be a string")
                        })
                    })
                    .transpose()?;
                let append_system_prompt = request
                    .get("appendSystemPrompt")
                    .map(|value| {
                        value.as_str().ok_or_else(|| {
                            anyhow::anyhow!("initialize appendSystemPrompt must be a string")
                        })
                    })
                    .transpose()?;
                // Parse the new SDK agent catalog without mutating the active
                // session. A schema error must not leak partially installed
                // agents into a subsequent initialize attempt.
                let prepared_agents = request
                    .get("agents")
                    .filter(|value| !value.is_null())
                    .map(|definitions| agent.prepare_sdk_agent_definitions(definitions))
                    .transpose()?;
                let schema = request
                    .get("jsonSchema")
                    .filter(|value| !value.is_null())
                    .cloned();
                agent.set_sdk_response_schema(schema)?;
                if let Some(prepared) = prepared_agents {
                    agent.install_sdk_agent_definitions(prepared);
                }
                let agents = agent.sdk_agent_descriptors()?;
                let available_output_styles =
                    prompt_runtime::available_output_styles(agent.workspace_root())?;
                let output_style =
                    std::env::var("ATLAS_OUTPUT_STYLE").unwrap_or_else(|_| "default".to_owned());
                // Existing lowercase environment values remain accepted; the
                // SDK announces the canonical 2.1.88 built-in style names.
                let output_style = match output_style.as_str() {
                    "explanatory" => "Explanatory".to_owned(),
                    "learning" => "Learning".to_owned(),
                    _ => output_style,
                };
                if !available_output_styles.contains(&output_style) {
                    anyhow::bail!("Configured output style is not available: {output_style}");
                }
                agent.set_sdk_system_prompts(system_prompt, append_system_prompt);
                *sdk_initialized = true;
                Ok(
                    json!({"commands":commands,"agents":agents,"output_style":output_style,
                    "available_output_styles":available_output_styles,"models":models,
                    "account":{},"pid":std::process::id()}),
                )
            }
            "reload_plugins" => {
                // Reload from disk: the SDK may have updated enabled plugins
                // after initialization. Preserve the live model and session.
                let refreshed = Config::load()?;
                let reload = agent.reload_plugins(&refreshed).await?;
                let commands = sdk_visible_commands(agent, &refreshed)?;
                let agents = agent.sdk_agent_descriptors()?;
                let mcp_servers = agent.mcp_statuses()?;
                let error_count = reload
                    .get("plugin_error_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize
                    + mcp_servers
                        .iter()
                        .filter(|server| {
                            server.get("status").and_then(Value::as_str) == Some("failed")
                        })
                        .count();
                let plugins = refreshed
                    .enabled_plugins
                    .iter()
                    .map(|name| {
                        json!({
                            "name":name,
                            "path":agent.workspace_root().join(".atlas/plugins")
                                .join(name).to_string_lossy(),
                            "source":"local"
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(json!({"commands":commands,"agents":agents,
                    "plugins":plugins,"mcpServers":mcp_servers,
                    "error_count":error_count}))
            }
            "mcp_set_servers" => {
                let servers = request
                    .get("servers")
                    .and_then(Value::as_object)
                    .ok_or_else(|| anyhow::anyhow!("mcp_set_servers requires servers object"))?;
                agent.set_dynamic_mcp_servers(servers).await
            }
            "mcp_toggle" => {
                let server = request
                    .get("serverName")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("mcp_toggle requires serverName"))?;
                let enabled = request
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| anyhow::anyhow!("mcp_toggle requires boolean enabled"))?;
                agent.toggle_named_mcp(server, enabled).await?;
                Ok(json!({}))
            }
            "mcp_reconnect" => {
                let server = request
                    .get("serverName")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("mcp_reconnect requires serverName"))?;
                agent.reconnect_named_mcp(server).await?;
                Ok(json!({}))
            }
            "mcp_status" => Ok(json!({"mcpServers":agent.mcp_statuses()?})),
            "get_context_usage" => agent.context_usage(),
            "get_settings" => {
                permission_settings::SettingsPaths::for_workspace(agent.workspace_root())?
                    .settings_snapshot(&agent.profile().model)
            }
            "mcp_prompt_commands" => agent.list_mcp_prompt_commands().await,
            "mcp_resources"
            | "mcp_prompts"
            | "mcp_resource_templates"
            | "mcp_read_resource"
            | "mcp_get_prompt"
            | "mcp_resource_subscribe"
            | "mcp_resource_unsubscribe"
            | "mcp_resource_updates"
            | "mcp_poll_resource_updates" => {
                mcp_control_request(agent, subtype, &Value::Object(request.clone())).await
            }
            "rewind_files" => {
                let id = request
                    .get("user_message_id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("rewind_files requires user_message_id"))?;
                let dry_run = match request.get("dry_run") {
                    None => false,
                    Some(Value::Bool(value)) => *value,
                    _ => bail!("rewind_files dry_run must be a boolean"),
                };
                agent.sdk_rewind_files(id, dry_run)
            }
            "generate_session_title" => {
                let description = request
                    .get("description")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow::anyhow!("generate_session_title requires description string")
                    })?;
                let persist = match request.get("persist") {
                    None => false,
                    Some(Value::Bool(value)) => *value,
                    _ => bail!("generate_session_title persist must be a boolean"),
                };
                let title = agent.generate_session_title(description, persist).await?;
                Ok(json!({"title":title}))
            }
            "seed_read_state" => {
                let path = request
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("seed_read_state requires path"))?;
                let mtime = request
                    .get("mtime")
                    .and_then(Value::as_f64)
                    .filter(|value| value.is_finite() && *value >= 0.0 && *value < u64::MAX as f64)
                    .ok_or_else(|| {
                        anyhow::anyhow!("seed_read_state requires a nonnegative finite mtime")
                    })?;
                agent.seed_read_state(path, mtime.floor() as u64)?;
                Ok(json!({}))
            }
            "cancel_async_message" => {
                let uuid = request
                    .get("message_uuid")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        anyhow::anyhow!("cancel_async_message requires non-empty message_uuid")
                    })?;
                // Original 2.1.88 uses dequeueAllMatching, not priority dequeue.
                // Already-dispatched messages cannot be retroactively cancelled.
                let removed = queue.dequeue_all_matching(|item| item.uuid.as_deref() == Some(uuid));
                Ok(json!({"cancelled":!removed.is_empty()}))
            }
            "set_max_thinking_tokens" => {
                // print.ts:2945-2956: null restores provider defaults,
                // zero disables thinking, and a positive integer is a budget.
                let budget = match request.get("max_thinking_tokens") {
                    Some(Value::Null) => None,
                    Some(Value::Number(number)) => {
                        let value = number
                            .as_u64()
                            .filter(|value| *value <= u32::MAX as u64)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "max_thinking_tokens must be a nonnegative integer within u32"
                                )
                            })?;
                        Some(value as u32)
                    }
                    _ => bail!("set_max_thinking_tokens requires max_thinking_tokens or null"),
                };
                agent.set_sdk_thinking_budget(budget)?;
                Ok(json!({}))
            }
            "set_model" => {
                // print.ts:2933-2944: omitted model also restores the default.
                let model = match request.get("model") {
                    None => "default",
                    Some(Value::String(model)) => model.as_str(),
                    Some(_) => bail!("set_model model must be a string"),
                };
                let profile = if model == "default" {
                    config.select(None, None)?
                } else {
                    config.select(Some(&agent.profile().id), Some(model))?
                };
                agent.sdk_switch_model(profile, model)?;
                Ok(json!({"model":agent.profile().model,"provider":agent.profile().id}))
            }
            "set_permission_mode" => {
                let mode = request
                    .get("mode")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("set_permission_mode requires mode"))?;
                agent.set_sdk_permission_mode(mode)?;
                Ok(json!({"mode":agent.current_permission_mode()}))
            }
            "stop_task" => {
                let task_id = request
                    .get("task_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("stop_task requires task_id"))?;
                let (_tool_result, description) =
                    agent.stop_background_task_for_sdk(task_id).await?;
                // stopTask.ts emits this event separately because suppressing
                // the noisy shell exit notification would otherwise leave SDK
                // clients showing a task as running after a successful stop.
                print_json!(json!({
                    "type":"system",
                    "subtype":"task_notification",
                    "task_id":task_id,
                    "status":"stopped",
                    "output_file":"",
                    "summary":description,
                    "session_id":agent.session_id(),
                    "uuid":uuid::Uuid::new_v4().to_string()
                }));
                // print.ts::stop_task acknowledges with an empty object;
                // the task's lifecycle is delivered by the event above.
                Ok(json!({}))
            }
            other => bail!("Unsupported SDK control subtype: {other}"),
        }
    }
    .await;
    match outcome {
        Ok(response) => json!({"type":"control_response","response":{
            "subtype":"success","request_id":id,"response":response
        }}),
        Err(error) => json!({"type":"control_response","response":{
            "subtype":"error","request_id":id,"error":format!("{error:#}")
        }}),
    }
}

/// Explicit SDK MCP introspection; never invoke arbitrary server methods.
async fn mcp_control_request(agent: &mut Agent, action: &str, request: &Value) -> Result<Value> {
    match action {
        "mcp_resources" => agent.mcp_catalog("resources").await,
        "mcp_prompts" => agent.mcp_catalog("prompts").await,
        "mcp_resource_templates" => agent.mcp_catalog("resource_templates").await,
        "mcp_resource_updates" => Ok(agent.mcp_resource_updates()),
        "mcp_poll_resource_updates" => {
            let max_wait_ms = match request.get("max_wait_ms") {
                None => 1000,
                Some(value) => value.as_u64().filter(|ms| *ms <= 30_000).ok_or_else(|| {
                    anyhow::anyhow!("MCP max_wait_ms must be an integer from 0 to 30000")
                })?,
            };
            Ok(agent
                .mcp_poll_resource_updates(std::time::Duration::from_millis(max_wait_ms))
                .await)
        }
        "mcp_resource_subscribe" | "mcp_resource_unsubscribe" => {
            let server = request
                .get("server")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("MCP subscription requires server"))?;
            let uri = request
                .get("uri")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("MCP subscription requires uri"))?;
            agent
                .mcp_subscribe_resource(server, uri, action == "mcp_resource_subscribe")
                .await
        }
        "mcp_read_resource" => {
            let server = request
                .get("server")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("mcp_read_resource requires server"))?;
            let uri = request
                .get("uri")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("mcp_read_resource requires uri"))?;
            agent.mcp_read_resource(server, uri).await
        }
        "mcp_get_prompt" => {
            let server = request
                .get("server")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("mcp_get_prompt requires server"))?;
            let name = request
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("mcp_get_prompt requires name"))?;
            let empty = serde_json::Map::new();
            let arguments = match request.get("arguments") {
                None | Some(Value::Null) => &empty,
                Some(Value::Object(args)) => args,
                _ => bail!("mcp_get_prompt arguments must be an object"),
            };
            agent.mcp_get_prompt(server, name, arguments).await
        }
        _ => bail!("Unknown MCP SDK operation"),
    }
}
