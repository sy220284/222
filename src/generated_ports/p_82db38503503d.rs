//! Rust exports derived from original: src/utils/computerUse/setup.ts
//!
//! Contract: setupComputerUseMCP builds the dynamic MCP server config and an
//! allowedTools list of `mcp__computer-use__*` names so they bypass the
//! normal permission prompt (request_access covers the whole session), and
//! the API backend detects those names to emit a CU availability hint into
//! the system prompt — built-in tools with other names would not trigger it.
//! The @ant/computer-use-mcp tool builder is vendor-native; the name
//! construction is fully equivalent here.

/// `buildMcpToolName(COMPUTER_USE_MCP_SERVER_NAME, tool)` — the wire prefix
/// that the backend keys on.
pub fn build_mcp_tool_name(server: &str, tool: &str) -> String {
    format!("mcp__{server}__{tool}")
}

pub const COMPUTER_USE_MCP_SERVER_NAME: &str = "computer-use";
