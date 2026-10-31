//! Rust exports derived from original: src/tasks.ts
//!
//! Task registry shape: the always-registered set plus the two feature-gated
//! entries (`WORKFLOW_SCRIPTS`, `MONITOR_TOOL`). The original returns the
//! array inline to avoid circular dependency issues; concrete task types are
//! ported with their own files.

/// `getAllTasks()` registry order: LocalShell, LocalAgent, RemoteAgent, Dream,
/// then the feature-gated LocalWorkflow and MonitorMcp entries.
pub fn get_all_tasks(
    feature_workflow_scripts: bool,
    feature_monitor_tool: bool,
) -> Vec<&'static str> {
    let mut tasks = vec![
        "LocalShellTask",
        "LocalAgentTask",
        "RemoteAgentTask",
        "DreamTask",
    ];
    if feature_workflow_scripts {
        tasks.push("LocalWorkflowTask");
    }
    if feature_monitor_tool {
        tasks.push("MonitorMcpTask");
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_order_and_gates() {
        assert_eq!(
            get_all_tasks(false, false),
            vec![
                "LocalShellTask",
                "LocalAgentTask",
                "RemoteAgentTask",
                "DreamTask"
            ]
        );
        assert_eq!(get_all_tasks(true, true).len(), 6);
    }
}
