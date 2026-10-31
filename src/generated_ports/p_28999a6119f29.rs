//! Rust exports derived from original:
//! src/components/messages/nullRenderingAttachments.ts
//!
//! Full equivalence: attachment types rendered as null unconditionally are
//! filtered before the render cap so invisible entries never consume the
//! 200-message render budget (CC-724). The original enforces sync via
//! TypeScript's `satisfies` on the switch default; here the list is the
//! single source queried by the render filter.

/// `NULL_RENDERING_TYPES` — verbatim list.
pub const NULL_RENDERING_TYPES: &[&str] = &[
    "hook_success",
    "hook_additional_context",
    "hook_cancelled",
    "command_permissions",
    "agent_mention",
    "budget_usd",
    "critical_system_reminder",
    "edited_image_file",
    "edited_text_file",
    "opened_file_in_ide",
    "output_style",
    "plan_mode",
    "plan_mode_exit",
    "plan_mode_reentry",
    "structured_output",
    "team_context",
    "todo",
    "task_progress",
    "generated_file",
    "-teammate-view",
];

/// Whether an attachment type renders null unconditionally.
pub fn is_null_rendering(attachment_type: &str) -> bool {
    NULL_RENDERING_TYPES.contains(&attachment_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership() {
        assert!(is_null_rendering("hook_success"));
        assert!(is_null_rendering("-teammate-view"));
        assert!(!is_null_rendering("bash-stdout"));
    }
}
