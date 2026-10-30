//! Rust exports derived from original: src/hooks/useIdeConnectionStatus.ts
//!
//! Full equivalence: locate the client named "ide", extract the IDE name only
//! for sse-ide/ws-ide transports, and map the connection type to a status.

/// `IdeStatus` — null means no IDE client present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeStatus {
    Connected,
    Disconnected,
    Pending,
}

/// `IdeConnectionResult` — status plus the display name when extractable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeConnectionResult {
    pub status: Option<IdeStatus>,
    pub ide_name: Option<String>,
}

/// Minimal client shape consumed by the derivation (name/type/config type /
/// config ide name).
pub struct IdeClient<'a> {
    pub name: &'a str,
    pub connection: ConnectionKind,
    pub config_ide_name: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Connected,
    Pending,
    Disconnected,
    SseIde,
    WsIde,
    Other,
}

/// `useIdeConnectionStatus` derivation over the client list.
pub fn ide_connection_status(clients: &[IdeClient<'_>]) -> IdeConnectionResult {
    let Some(client) = clients.iter().find(|c| c.name == "ide") else {
        return IdeConnectionResult {
            status: None,
            ide_name: None,
        };
    };
    let ide_name = match client.connection {
        ConnectionKind::SseIde | ConnectionKind::WsIde => client.config_ide_name.map(str::to_owned),
        _ => None,
    };
    let status = match client.connection {
        ConnectionKind::Connected => Some(IdeStatus::Connected),
        ConnectionKind::Disconnected => Some(IdeStatus::Disconnected),
        ConnectionKind::Pending => Some(IdeStatus::Pending),
        _ => None,
    };
    IdeConnectionResult { status, ide_name }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_ide_client_yields_null() {
        let r = ide_connection_status(&[]);
        assert_eq!(
            r,
            IdeConnectionResult {
                status: None,
                ide_name: None
            }
        );
    }

    #[test]
    fn connected_sse_ide_extracts_name() {
        let clients = [IdeClient {
            name: "ide",
            connection: ConnectionKind::SseIde,
            config_ide_name: Some("VS Code"),
        }];
        let r = ide_connection_status(&clients);
        assert_eq!(r.status, Some(IdeStatus::Connected));
        assert_eq!(r.ide_name.as_deref(), Some("VS Code"));
    }

    #[test]
    fn other_transport_has_no_name() {
        let clients = [IdeClient {
            name: "ide",
            connection: ConnectionKind::Connected,
            config_ide_name: Some("VS Code"),
        }];
        let r = ide_connection_status(&clients);
        assert_eq!(r.status, Some(IdeStatus::Connected));
        assert_eq!(r.ide_name, None);
    }
}
