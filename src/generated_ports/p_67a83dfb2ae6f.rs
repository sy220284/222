//! Rust exports derived from original: src/hooks/useIdeConnectionStatus.ts
//!
//! Full equivalence: the original matches the client named "ide" and derives
//! status from the connection type (connected/disconnected/pending) while
//! the display name comes from the transport config (sse-ide/ws-ide) — two
//! independent dimensions, not one enum.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeStatus {
    Connected,
    Disconnected,
    Pending,
}

/// `config.type` — sse-ide / ws-ide carry an IDE display name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeTransport {
    SseIde,
    WsIde,
    Other,
}

/// `client.type` — connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Connected,
    Disconnected,
    Pending,
    Failed,
}

pub struct IdeClient<'a> {
    pub name: &'a str,
    pub connection: ConnectionKind,
    pub transport: IdeTransport,
    pub config_ide_name: Option<&'a str>,
}

pub struct IdeConnectionResult {
    pub status: Option<IdeStatus>,
    pub ide_name: Option<String>,
}

/// `useIdeConnectionStatus` semantic core.
pub fn ide_connection_status(clients: &[IdeClient<'_>]) -> IdeConnectionResult {
    let Some(client) = clients.iter().find(|c| c.name == "ide") else {
        return IdeConnectionResult {
            status: None,
            ide_name: None,
        };
    };
    let ide_name = match client.transport {
        IdeTransport::SseIde | IdeTransport::WsIde => client.config_ide_name.map(str::to_owned),
        IdeTransport::Other => None,
    };
    let status = match client.connection {
        ConnectionKind::Connected => Some(IdeStatus::Connected),
        ConnectionKind::Disconnected => Some(IdeStatus::Disconnected),
        ConnectionKind::Pending => Some(IdeStatus::Pending),
        ConnectionKind::Failed => None,
    };
    IdeConnectionResult { status, ide_name }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_ide_client_yields_null() {
        let r = ide_connection_status(&[]);
        assert_eq!(r.status, None);
        assert_eq!(r.ide_name, None);
    }

    #[test]
    fn connected_sse_ide_extracts_name() {
        let clients = [IdeClient {
            name: "ide",
            connection: ConnectionKind::Connected,
            transport: IdeTransport::SseIde,
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
            transport: IdeTransport::Other,
            config_ide_name: Some("VS Code"),
        }];
        let r = ide_connection_status(&clients);
        assert_eq!(r.status, Some(IdeStatus::Connected));
        assert_eq!(r.ide_name, None);
    }

    #[test]
    fn failed_connection_reads_null() {
        let clients = [IdeClient {
            name: "ide",
            connection: ConnectionKind::Failed,
            transport: IdeTransport::WsIde,
            config_ide_name: Some("VS Code"),
        }];
        let r = ide_connection_status(&clients);
        assert_eq!(r.status, None);
    }
}
