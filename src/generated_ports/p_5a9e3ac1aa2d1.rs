//! Rust exports derived from original: src/hooks/notifs/useMcpConnectivityStatus.tsx
//!
//! Notification trigger: warn once when a hosted MCP server connects but
//! the built-in feature flag has never connected (startup race). Remote
//! mode and empty client lists never notify.

/// Whether the connectivity warning should fire for this tick.
pub fn should_warn(
    ever_connected: bool,
    remote_mode: bool,
    hosted_mcp_server_present: bool,
) -> bool {
    hosted_mcp_server_present && !ever_connected && !remote_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_matrix() {
        assert!(should_warn(false, false, true));
        assert!(!should_warn(true, false, true));
        assert!(!should_warn(false, true, true));
        assert!(!should_warn(false, false, false));
    }
}
