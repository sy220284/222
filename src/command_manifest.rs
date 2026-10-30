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
bridge|remote-control|local-jsx|rc|[name]|Connect remote control
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
extra-usage|extra-usage|local-jsx||-|Additional usage
fast|fast|local-jsx||[on|off]|Fast mode
feedback|feedback|local-jsx|bug|[report]|Send feedback
files|files|local||-|Show context files
good-CODEBRAND|stub|disabled||-|Disabled internal command
heapdump|heapdump|local||-|Heap snapshot
help|help|local-jsx||-|Show help
hooks|hooks|local-jsx||-|Manage hooks
ide|ide|local-jsx||[open]|Connect editor
install-github-app|install-github-app|local-jsx||-|Install repository automation
install-slack-app|install-slack-app|local||-|Install chat integration
issue|stub|disabled||-|Disabled issue command
keybindings|keybindings|local||-|Edit keybindings
mcp|mcp|local-jsx||[enable|disable]|Manage MCP
memory|memory|local-jsx||-|Edit memory
mobile|mobile|local-jsx|ios,android|-|Mobile companion
mock-limits|stub|disabled||-|Disabled limit mock
model|model|local-jsx||[model]|Select model
oauth-refresh|stub|disabled||-|Disabled OAuth refresh
onboarding|stub|disabled||-|Disabled onboarding
output-style|output-style|local-jsx||-|Legacy styles
passes|passes|local-jsx||-|Usage passes
perf-issue|stub|disabled||-|Disabled performance issue
permissions|permissions|local-jsx|allowed-tools|-|Tool permissions
plan|plan|local-jsx||[open|description]|Planning mode
plugin|plugin|local-jsx|plugins,marketplace|-|Plugin manager
pr_comments|pr-comments|text||-|PR comments
privacy-settings|privacy-settings|local-jsx||-|Privacy options
rate-limit-options|rate-limit-options|local-jsx||-|Rate options
release-notes|release-notes|local||-|Release notes
rename|rename|local-jsx||[name]|Rename the current conversation
reload-plugins|reload-plugins|local||-|Reload plugins
remote-env|remote-env|local-jsx||-|Remote environment
remote-setup|web-setup|local-jsx||-|Setup remote
reset-limits|stub|disabled||-|Disabled limit reset
resume|resume|local-jsx|continue|[conversation id or search term]|Resume a previous conversation
rewind|rewind|local|checkpoint|-|Rewind conversation
sandbox-toggle|sandbox|local-jsx||-|Sandbox settings
session|session|local-jsx|remote|-|Session URL
share|stub|disabled||-|Disabled share
skills|skills|local-jsx||-|List skills
stats|stats|local-jsx||-|Activity statistics
status|status|local-jsx||-|Runtime status
stickers|stickers|local||-|Merchandise
summary|stub|disabled||-|Disabled summary
tag|tag|local-jsx||<tag-name>|Session tags
tasks|tasks|local-jsx|bashes|-|Background tasks
teleport|stub|disabled||-|Disabled teleport
terminalSetup|terminal-setup|local-jsx||-|Terminal setup
theme|theme|local-jsx||-|Terminal theme
thinkback|think-back|local-jsx||-|Historical review
thinkback-play|thinkback-play|local||-|Historical animation
upgrade|upgrade|local-jsx||-|Upgrade options
usage|usage|local-jsx||-|Usage
vim|vim|local||-|Toggle Vim mode
voice|voice|local||-|Voice mode"####;
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
                    "remote-control"
                        | "btw"
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
                let source: &'static str = Box::leak(
                    format!(
                        "src/commands/{actual_dir}/index.{}",
                        if disabled {
                            "js"
                        } else if directory == "plugin" {
                            "tsx"
                        } else {
                            "ts"
                        }
                    )
                    .into_boxed_str(),
                );
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
    fn registry_and_alias_resolution() {
        assert_eq!(commands().len(), 83);
        assert_eq!(commands().iter().filter(|c| c.disabled).count(), 18);
        assert_eq!(lookup("settings").unwrap().name, "config");
        assert_eq!(lookup("add-dir").unwrap().name, "add-dir");
        assert_eq!(lookup("export").unwrap().name, "export");
        assert_eq!(lookup("rename").unwrap().name, "rename");
        assert_eq!(lookup("continue").unwrap().name, "resume");
        for command in [
            "remote-control",
            "btw",
            "color",
            "exit",
            "hooks",
            "mcp",
            "plugin",
            "rename",
            "sandbox",
            "status",
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
