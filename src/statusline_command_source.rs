//! Hand port of src/commands/statusline.tsx's prompt command definition.
//! The bundled agent identifier and settings path are provider-neutral here.
use serde_json::{json, Value};

pub struct StatuslineCommand {
    pub command_type: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub progress_message: &'static str,
    pub content_length: usize,
    pub aliases: &'static [&'static str],
    pub allowed_tools: &'static [&'static str],
    pub source: &'static str,
    pub disable_noninteractive: bool,
}

pub const STATUSLINE_COMMAND: StatuslineCommand = StatuslineCommand {
    command_type: "prompt",
    name: "statusline",
    description: "Set up the terminal status line UI",
    progress_message: "setting up statusLine",
    content_length: 0,
    aliases: &[],
    allowed_tools: &[
        "Agent",
        "Read(~/**)",
        "Edit(~/.config/atlas-agent/config.json)",
    ],
    source: "builtin",
    disable_noninteractive: true,
};

/// Return the original command's single content block. The prompt command
/// intentionally supplies text to the model, without initiating agents here.
pub fn get_prompt_for_command(args: &str) -> Vec<Value> {
    let prompt = if args.trim().is_empty() {
        "Configure my statusLine from my shell PS1 configuration"
    } else {
        args.trim()
    };
    vec![json!({
        "type": "text",
        "text": format!("Create an Agent with subagent_type \"statusline-setup\" and the prompt \"{prompt}\"")
    })]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_and_custom_instruction_preserve_original_prompt_contract() {
        assert_eq!(STATUSLINE_COMMAND.command_type, "prompt");
        assert_eq!(STATUSLINE_COMMAND.name, "statusline");
        assert!(STATUSLINE_COMMAND.disable_noninteractive);
        assert_eq!(STATUSLINE_COMMAND.content_length, 0);
        assert_eq!(STATUSLINE_COMMAND.aliases, &[] as &[&str]);
        assert_eq!(
            get_prompt_for_command(" "),
            vec![json!({
                "type":"text",
                "text":"Create an Agent with subagent_type \"statusline-setup\" and the prompt \"Configure my statusLine from my shell PS1 configuration\""
            })]
        );
        assert_eq!(get_prompt_for_command("  show branch and model  ")[0]["text"],
            "Create an Agent with subagent_type \"statusline-setup\" and the prompt \"show branch and model\"");
    }
}
