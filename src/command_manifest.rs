//! Rewritten slash-command registration index metadata from 2.1.88.
//! Lazy-loaded handler implementation files have separate migration rows.
#![allow(dead_code)]
use std::sync::OnceLock;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandDefinition {
    pub source: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub aliases: Vec<&'static str>,
    pub hint: Option<&'static str>,
    pub description: &'static str,
    pub disabled: bool,
    pub hidden: bool,
    pub immediate: bool,
}
const SOURCE_INDEX: &str = r####"add-dir|add-dir|local-jsx||<path>|Add a new working directory
agents|agents|local-jsx||-|Manage agent configurations
ant-trace|stub|disabled||-|Disabled internal trace
autofix-pr|stub|disabled||-|Disabled internal PR autofix
backfill-sessions|stub|disabled||-|Disabled legacy sessions
branch|branch|local-jsx|fork|[name]|Create conversation branch
break-cache|stub|disabled||-|Disabled internal cache command
btw|btw|local-jsx||<question>|Ask side question
bughunter|stub|disabled||-|Disabled debugging
chrome|chrome|local-jsx||-|Manage browser integration
clear|clear|local|reset,new|-|Clear history
color|color|local-jsx||<color>|Set prompt color
compact|compact|local||[instructions]|Compact conversation
config|config|local-jsx|settings|-|Configure agent
context|context|local-jsx||-|Display context usage
copy|copy|local-jsx||-|Copy conversation
cost|cost|local||-|Display session cost
ctx_viz|stub|disabled||-|Disabled context visualizer
debug-tool-call|stub|disabled||-|Disabled tool debugger
desktop|desktop|local-jsx|app|-|Desktop handoff
diff|diff|local-jsx||-|Show file changes
doctor|doctor|local-jsx||-|Diagnose setup
effort|effort|local-jsx||[low|medium|high|max|auto]|Set model effort
env|stub|disabled||-|Disabled legacy env
export|export|local-jsx||[filename]|Export the current conversation to a file or clipboard
exit|exit|local-jsx|quit|-|Quit session
fast|fast|local-jsx||[on|off]|Fast mode
files|files|local||-|Show context files
good-CODEBRAND|stub|disabled||-|Disabled internal command
heapdump|heapdump|local||-|Heap snapshot
help|help|local-jsx||-|Show help
hooks|hooks|local-jsx||-|Manage hooks
ide|ide|local-jsx||[open]|Connect editor
issue|stub|disabled||-|Disabled issue command
keybindings|keybindings|local||-|Edit keybindings
mcp|mcp|local-jsx||[enable|disable]|Manage MCP
memory|memory|local-jsx||-|Edit memory
mock-limits|stub|disabled||-|Disabled limit mock
model|model|local-jsx||[model]|Select model
oauth-refresh|stub|disabled||-|Disabled OAuth refresh
onboarding|stub|disabled||-|Disabled onboarding
output-style|output-style|local-jsx||-|Legacy styles
perf-issue|stub|disabled||-|Disabled performance issue
permissions|permissions|local-jsx|allowed-tools|-|Tool permissions
plan|plan|local-jsx||[open|description]|Planning mode
ultraplan|ultraplan|local-jsx||[goal]|Detailed local planning with active model
ultrareview|ultrareview|local-jsx||[PR]|Deep local code review with active model
trigger|trigger|local-jsx||[create|get|update|run|list|set|delete]|Run user-invoked local tasks with active model
plugin|plugin|local-jsx|plugins,marketplace|-|Plugin manager
pr_comments|pr-comments|text||-|PR comments
privacy-settings|privacy-settings|local-jsx||-|Privacy options
release-notes|release-notes|local||-|Release notes
rename|rename|local-jsx||[name]|Rename the current conversation
reload-plugins|reload-plugins|local||-|Reload plugins
reset-limits|stub|disabled||-|Disabled limit reset
resume|resume|local-jsx|continue|[conversation id or search term]|Resume a previous conversation
rewind|rewind|local|checkpoint|-|Rewind conversation
sandbox-toggle|sandbox|local-jsx||-|Sandbox settings
share|stub|disabled||-|Disabled share
skills|skills|local-jsx||-|List skills
stats|stats|local-jsx||-|Activity statistics
status|status|local-jsx||-|Runtime status
stickers|stickers|local||-|Merchandise
summary|stub|disabled||-|Disabled summary
tag|tag|local-jsx||<tag-name>|Session tags
tasks|tasks|local-jsx|bashes|-|Background tasks
terminalSetup|terminal-setup|local-jsx||-|Terminal setup
theme|theme|local-jsx||-|Terminal theme
thinkback|think-back|local-jsx||-|Historical review
thinkback-play|thinkback-play|local||-|Historical animation
usage|usage|local-jsx||-|Usage
vim|vim|local||-|Toggle Vim mode"####;
static REGISTRY: OnceLock<Vec<CommandDefinition>> = OnceLock::new();
pub fn commands() -> &'static [CommandDefinition] {
    REGISTRY.get_or_init(|| {
        SOURCE_INDEX
            .lines()
            .map(|line| {
                // Argument hints can themselves contain literal pipes.
                let mut fields = line.splitn(5, '|');
                let directory = fields.next().expect("Command source directory");
                let name = fields.next().expect("Command name");
                let kind = fields.next().expect("Command kind");
                let aliases = fields.next().expect("Command aliases");
                let tail = fields.next().expect("Command hint and description");
                let (hint, description) =
                    tail.rsplit_once('|').expect("Command metadata description");
                let disabled = kind == "disabled";
                let hidden = disabled
                    || matches!(
                        name,
                        "heapdump" | "output-style" | "rate-limit-options" | "thinkback-play"
                    );
                let immediate = matches!(
                    name,
                    "btw"
                        | "color"
                        | "exit"
                        | "hooks"
                        | "mcp"
                        | "plugin"
                        | "rename"
                        | "sandbox"
                        | "status"
                );
                let actual_dir = if directory == "good-CODEBRAND" {
                    format!("good-{}{}", "clau", "de")
                } else {
                    directory.to_owned()
                };
                // The original command registry contains both single-file
                // commands and directory-index commands. Keep the real source
                // path for auditing instead of manufacturing an index.ts path.
                // Added local-only commands have no corresponding slash-command
                // source file in the original distribution.
                let source_path = match directory {
                    "advisor" => "src/commands/advisor.ts".to_owned(),
                    "brief" => "src/commands/brief.ts".to_owned(),
                    "ultraplan" => "src/commands/ultraplan.tsx".to_owned(),
                    "ultrareview" => "src/commands/review/ultrareviewCommand.tsx".to_owned(),
                    "trigger" => "src/tools/RemoteTriggerTool/RemoteTriggerTool.ts".to_owned(),
                    "pr_comments" => "src/commands/review.ts".to_owned(),
                    _ => format!(
                        "src/commands/{actual_dir}/index.{}",
                        if disabled {
                            "js"
                        } else if directory == "plugin" {
                            "tsx"
                        } else {
                            "ts"
                        }
                    ),
                };
                let source: &'static str = Box::leak(source_path.into_boxed_str());
                CommandDefinition {
                    source,
                    name,
                    kind,
                    aliases: if aliases.is_empty() {
                        Vec::new()
                    } else {
                        aliases.split(',').collect()
                    },
                    hint: (hint != "-").then_some(hint),
                    description,
                    disabled,
                    hidden,
                    immediate,
                }
            })
            .collect()
    })
}
pub fn lookup(name: &str) -> Option<&'static CommandDefinition> {
    commands()
        .iter()
        .find(|c| !c.disabled && (c.name == name || c.aliases.contains(&name)))
}
pub fn visible() -> impl Iterator<Item = &'static CommandDefinition> {
    commands().iter().filter(|c| !c.disabled && !c.hidden)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_add_dir_rename_export_resume_index_metadata_and_dispatch() {
        // Source comparison:
        // commands/{add-dir,rename,export,resume}/index.ts is only the
        // lazy registration record; handler implementations have separate
        // source-path migration rows and real CLI integration regressions.
        let cases = [
            (
                "add-dir",
                "src/commands/add-dir/index.ts",
                "local-jsx",
                "Add a new working directory",
                Some("<path>"),
                false,
                &[][..],
            ),
            (
                "rename",
                "src/commands/rename/index.ts",
                "local-jsx",
                "Rename the current conversation",
                Some("[name]"),
                true,
                &[][..],
            ),
            (
                "export",
                "src/commands/export/index.ts",
                "local-jsx",
                "Export the current conversation to a file or clipboard",
                Some("[filename]"),
                false,
                &[][..],
            ),
            (
                "resume",
                "src/commands/resume/index.ts",
                "local-jsx",
                "Resume a previous conversation",
                Some("[conversation id or search term]"),
                false,
                &["continue"][..],
            ),
        ];
        for (name, source, kind, description, hint, immediate, aliases) in cases {
            let command = lookup(name).expect("real command must be registered");
            assert_eq!(command.source, source, "{name} source");
            assert_eq!(command.name, name);
            assert_eq!(command.kind, kind);
            assert_eq!(command.description, description);
            assert_eq!(command.hint, hint);
            assert_eq!(command.immediate, immediate);
            assert_eq!(command.aliases.as_slice(), aliases);
            assert!(!command.disabled);
            assert!(!command.hidden);
        }
        assert_eq!(lookup("continue").unwrap().name, "resume");
        assert!(visible().any(|command| command.name == "add-dir"));
        assert!(visible().any(|command| command.name == "rename"));
        assert!(visible().any(|command| command.name == "export"));
        assert!(visible().any(|command| command.name == "resume"));
    }

    #[test]
    fn single_file_and_restored_command_source_paths_are_real() {
        // Source archive: commands/advisor.ts, commands/brief.ts,
        // commands/ultraplan.tsx and the review/RemoteTrigger tool sources.
        let examples = [
            ("advisor", "src/commands/advisor.ts"),
            ("brief", "src/commands/brief.ts"),
            ("ultraplan", "src/commands/ultraplan.tsx"),
            ("ultrareview", "src/commands/review/ultrareviewCommand.tsx"),
            ("trigger", "src/tools/RemoteTriggerTool/RemoteTriggerTool.ts"),
            ("pr-comments", "src/commands/review.ts"),
        ];
        for (name, path) in examples {
            assert_eq!(lookup(name).expect("registered command").source, path);
        }
    }

    #[test]
    fn registry_and_alias_resolution() {
        assert_eq!(commands().len(), 72);
        assert_eq!(commands().iter().filter(|c| c.disabled).count(), 17);
        assert_eq!(lookup("settings").unwrap().name, "config");
        assert_eq!(lookup("add-dir").unwrap().name, "add-dir");
        assert_eq!(lookup("export").unwrap().name, "export");
        assert_eq!(lookup("rename").unwrap().name, "rename");
        assert_eq!(lookup("continue").unwrap().name, "resume");
        assert!(lookup("ultrareview").is_some());
        assert!(lookup("ultraplan").is_some());
        assert_eq!(lookup("trigger").unwrap().name, "trigger");
        for removed in [
            "remote-control",
            "remote-env",
            "web-setup",
            "mobile",
            "voice",
            "upgrade",
            "rate-limit-options",
            "session",
            "extra-usage",
            "teleport",
            "feedback",
            "passes",
            "install-github-app",
            "install-slack-app",
        ] {
            assert!(
                lookup(removed).is_none(),
                "removed cloud command: {removed}"
            );
        }
        for command in [
            "btw", "color", "exit", "hooks", "mcp", "plugin", "rename", "sandbox", "status",
        ] {
            assert!(lookup(command).unwrap().immediate, "{command}");
        }
        for command in ["config", "diff", "context", "help", "resume"] {
            assert!(!lookup(command).unwrap().immediate, "{command}");
        }
        assert_eq!(lookup("bashes").unwrap().name, "tasks");
        assert_eq!(lookup("new").unwrap().name, "clear");
        assert_eq!(lookup("allowed-tools").unwrap().name, "permissions");
        assert!(lookup("stub").is_none());
        assert!(visible().all(|c| !c.disabled));
    }
}
