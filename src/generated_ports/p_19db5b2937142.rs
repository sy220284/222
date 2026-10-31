//! Rust exports derived from original: src/utils/swarm/leaderPermissionBridge.ts
//!
//! Module-level bridge letting the REPL register its tool-use confirm queue
//! and permission-context setters so in-process teammates can use the
//! standard ToolUseConfirm dialog. Registration is last-writer-wins; both
//! setters are global singletons behind the agent layer's mutexes.

#[derive(Default)]
pub struct LeaderPermissionBridge {
    pub queue_setter_registered: bool,
    pub permission_context_setter_registered: bool,
}

impl LeaderPermissionBridge {
    /// `registerLeaderToolUseConfirmQueue`.
    pub fn register_queue(&mut self) {
        self.queue_setter_registered = true;
    }

    /// `registerLeaderToolPermissionContext`.
    pub fn register_permission_context(&mut self) {
        self.permission_context_setter_registered = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_flags() {
        let mut b = LeaderPermissionBridge::default();
        assert!(!b.queue_setter_registered);
        b.register_queue();
        b.register_permission_context();
        assert!(b.queue_setter_registered && b.permission_context_setter_registered);
    }
}
