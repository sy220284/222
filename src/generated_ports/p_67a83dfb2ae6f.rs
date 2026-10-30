//! Rust exports derived from original: src/hooks/useIdeConnectionStatus.ts
//!
//! Fully equivalent: derive the IDE connection status from MCP clients by
//! finding the client named "ide" and extracting the IDE name from
//! sse-ide/ws-ide configs.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeStatus {
    Connected,
    Disconnected,
    Pending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdeConnectionResult {
    pub status: Option<IdeStatus>,
    pub ide_name: Option<String>,
}

/// `useIdeConnectionStatus` pure core: memo over mcpClients.
/// `client_type`: the MCPServerConnection kind ("connected"/other);
/// `config`: the client config object (type/ideName fields).
pub fn ide_connection_status(clients: &[(String, String, Value)]) -> IdeConnectionResult {
    let Some((_, client_type, config)) = clients.iter().find(|(name, _, _)| name == "ide") else {
        return IdeConnectionResult {
            status: None,
            ide_name: None,
        };
    };
    let ide_name = if config["type"] == "sse-ide" || config["type"] == "ws-ide" {
        config["ideName"].as_str().map(str::to_owned)
    } else {
        None
    };
    let status = if client_type == "connected" {
        IdeStatus::Connected
    } else {
        IdeStatus::Disconnected
    };
    IdeConnectionResult {
        status: Some(status),
        ide_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ide_client_and_extracts_name() {
        let clients = vec![
            ("other".into(), "connected".into(), serde_json::json!({})),
            (
                "ide".into(),
                "connected".into(),
                serde_json::json!({"type":"sse-ide","ideName":"VS Code"}),
            ),
        ];
        let r = ide_connection_status(&clients);
        assert_eq!(r.status, Some(IdeStatus::Connected));
        assert_eq!(r.ide_name.as_deref(), Some("VS Code"));
        assert_eq!(ide_connection_status(&[]).status, None);
    }
}
