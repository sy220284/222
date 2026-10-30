//! Public, provider-neutral names of coding tools.
pub const BASH_TOOL_NAME: &str = "Bash";
pub const CONFIG_TOOL_NAME: &str = "Config";
pub const ENTER_PLAN_MODE_TOOL_NAME: &str = "EnterPlanMode";
pub const EXIT_PLAN_MODE_TOOL_NAME: &str = "ExitPlanMode";
pub const ENTER_WORKTREE_TOOL_NAME: &str = "EnterWorktree";
pub const EXIT_WORKTREE_TOOL_NAME: &str = "ExitWorktree";
pub const NOTEBOOK_EDIT_TOOL_NAME: &str = "NotebookEdit";
pub const TASK_OUTPUT_TOOL_NAME: &str = "TaskOutput";
pub const TASK_CREATE_TOOL_NAME: &str = "TaskCreate";
pub const TASK_GET_TOOL_NAME: &str = "TaskGet";
pub const TASK_LIST_TOOL_NAME: &str = "TaskList";
pub const TASK_UPDATE_TOOL_NAME: &str = "TaskUpdate";
pub const TEAM_CREATE_TOOL_NAME: &str = "TeamCreate";
pub const TEAM_DELETE_TOOL_NAME: &str = "TeamDelete";
pub const SEND_MESSAGE_TOOL_NAME: &str = "SendMessage";
pub const SKILL_TOOL_NAME: &str = "Skill";
pub const POWERSHELL_TOOL_NAME: &str = "PowerShell";
pub const TOOL_SEARCH_TOOL_NAME: &str = "ToolSearch";
pub const TODO_WRITE_TOOL_NAME: &str = "TodoWrite";
pub const EXIT_PLAN_MODE_V2_TOOL_NAME: &str = EXIT_PLAN_MODE_TOOL_NAME;
pub fn canonical_name(name: &str) -> &str {
    match name {
        BASH_TOOL_NAME => "run_command",
        "Read" => "read_file",
        "Edit" => "edit_file",
        "Write" => "write_file",
        NOTEBOOK_EDIT_TOOL_NAME => "notebook_edit",
        TASK_OUTPUT_TOOL_NAME | "AgentOutputTool" | "BashOutputTool" => "task_output",
        "TaskStop" => "task_stop",
        "Glob" => "glob_files",
        "Grep" => "grep_files",
        "WebFetch" => "web_fetch",
        "WebSearch" => "web_search",
        _ => name,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_aliases() {
        let cases: &[(&str, &str)] = &[
            (BASH_TOOL_NAME, "Bash"),
            (CONFIG_TOOL_NAME, "Config"),
            (ENTER_PLAN_MODE_TOOL_NAME, "EnterPlanMode"),
            (EXIT_PLAN_MODE_TOOL_NAME, "ExitPlanMode"),
            (ENTER_WORKTREE_TOOL_NAME, "EnterWorktree"),
            (EXIT_WORKTREE_TOOL_NAME, "ExitWorktree"),
            (NOTEBOOK_EDIT_TOOL_NAME, "NotebookEdit"),
            (TASK_OUTPUT_TOOL_NAME, "TaskOutput"),
            (TASK_CREATE_TOOL_NAME, "TaskCreate"),
            (TASK_GET_TOOL_NAME, "TaskGet"),
            (TASK_LIST_TOOL_NAME, "TaskList"),
            (TASK_UPDATE_TOOL_NAME, "TaskUpdate"),
            (TEAM_CREATE_TOOL_NAME, "TeamCreate"),
            (TEAM_DELETE_TOOL_NAME, "TeamDelete"),
            (SEND_MESSAGE_TOOL_NAME, "SendMessage"),
            (SKILL_TOOL_NAME, "Skill"),
            (POWERSHELL_TOOL_NAME, "PowerShell"),
            (TOOL_SEARCH_TOOL_NAME, "ToolSearch"),
            (TODO_WRITE_TOOL_NAME, "TodoWrite"),
        ];
        assert_eq!(cases.len(), 19);
        for (actual, expected) in cases {
            assert_eq!(actual, expected);
        }
        assert_eq!(EXIT_PLAN_MODE_V2_TOOL_NAME, "ExitPlanMode");
        assert_eq!(canonical_name("Bash"), "run_command");
        assert_eq!(canonical_name("NotebookEdit"), "notebook_edit");
        assert_eq!(canonical_name("TaskOutput"), "task_output");
        assert_eq!(canonical_name("TaskStop"), "task_stop");
        assert_eq!(canonical_name("Glob"), "glob_files");
        assert_eq!(canonical_name("Grep"), "grep_files");
        assert_eq!(canonical_name("WebSearch"), "web_search");
        assert_eq!(canonical_name("Config"), "Config");
    }
}
