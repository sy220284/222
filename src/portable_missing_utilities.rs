//! Original 2.1.88 independent utility behavior, reimplemented in provider-neutral Rust.
#![allow(dead_code)]
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::Path,
};

// src/utils/bash/registry.ts + src/utils/bash/specs/{index,alias,time,timeout}.ts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArgumentSpec {
    pub name: String,
    pub description: String,
    pub optional: bool,
    pub variadic: bool,
    pub is_command: bool,
    pub is_script: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub arguments: Vec<ArgumentSpec>,
}
fn arg(
    name: &str,
    description: &str,
    optional: bool,
    variadic: bool,
    is_command: bool,
) -> ArgumentSpec {
    ArgumentSpec {
        name: name.into(),
        description: description.into(),
        optional,
        variadic,
        is_command,
        is_script: false,
    }
}
pub fn builtin_command_specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            name: "timeout".into(),
            description: "Run a command with a time limit".into(),
            arguments: vec![
                arg(
                    "duration",
                    "Duration to wait before timing out (e.g., 10, 5s, 2m)",
                    false,
                    false,
                    false,
                ),
                arg("command", "Command to run", false, false, true),
            ],
        },
        CommandSpec {
            name: "sleep".into(),
            description: "Delay for a specified amount of time".into(),
            arguments: vec![arg(
                "duration",
                "Duration to sleep (seconds or with suffix like 5s, 2m, 1h)",
                false,
                false,
                false,
            )],
        },
        CommandSpec {
            name: "alias".into(),
            description: "Create or list command aliases".into(),
            arguments: vec![arg(
                "definition",
                "Alias definition in the form name=value",
                true,
                true,
                false,
            )],
        },
        CommandSpec {
            name: "nohup".into(),
            description: "Run a command immune to hangups".into(),
            arguments: vec![arg("command", "Command to run", false, false, true)],
        },
        CommandSpec {
            name: "time".into(),
            description: "Time a command".into(),
            arguments: vec![arg("command", "Command to time", false, false, true)],
        },
        crate::generated_ports::p_4007591beaebd::srun_spec(),
        crate::generated_ports::p_bec23e04adcfb::pyright_spec(),
    ]
}
pub fn valid_command_spec_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && (!name.starts_with('-') || name == "-")
}
pub fn get_builtin_command_spec(name: &str) -> Option<CommandSpec> {
    if !valid_command_spec_name(name) {
        return None;
    }
    builtin_command_specs().into_iter().find(|x| x.name == name)
}

// src/utils/directMemberMessage.ts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectMemberMessage {
    pub recipient: String,
    pub message: String,
}
pub fn parse_direct_member_message(input: &str) -> Option<DirectMemberMessage> {
    let rest = input.strip_prefix('@')?;
    let split_at = rest.char_indices().find(|(_, c)| c.is_whitespace())?.0;
    let recipient = &rest[..split_at];
    if recipient.is_empty()
        || !recipient
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let message = rest[split_at..].trim();
    if message.is_empty() {
        return None;
    }
    Some(DirectMemberMessage {
        recipient: recipient.into(),
        message: message.into(),
    })
}
pub fn send_direct_member_message(
    root: &Path,
    team: Option<&str>,
    recipient: &str,
    message: &str,
) -> Result<Value> {
    let Some(team) = team else {
        return Ok(json!({"success":false,"error":"no_team_context"}));
    };
    let registry = crate::team_registry::Teams::new(root)?;
    let Some(config) = registry.read(team)? else {
        return Ok(json!({"success":false,"error":"no_team_context"}));
    };
    if !config
        .members
        .iter()
        .any(|member| member.name == recipient && member.active)
    {
        return Ok(json!({"success":false,"error":"unknown_recipient","recipientName":recipient}));
    }
    let mailbox = crate::team_mailbox::Mailbox::new(root, team)?;
    mailbox.write(
        recipient,
        crate::team_mailbox::Message {
            from: "user".into(),
            text: message.into(),
            read: false,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            color: None,
            summary: None,
        },
    )?;
    Ok(json!({"success":true,"recipientName":recipient}))
}

// src/utils/highlightMatch.tsx
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchSpan {
    pub text: String,
    pub inverse: bool,
}
/// Preserve the original highlightMatch.tsx indexing: JS searches the entire
/// lowercased string, then slices the *original* string using UTF-16 offsets.
/// Lowercasing can expand a Unicode scalar, so independent character scans
/// do not reproduce the original offsets or the overlapping-match behavior.
pub fn highlight_match(text: &str, query: &str) -> Vec<MatchSpan> {
    if query.is_empty() {
        return vec![MatchSpan {
            text: text.into(),
            inverse: false,
        }];
    }
    let original: Vec<u16> = text.encode_utf16().collect();
    let normalized: Vec<u16> = text.to_lowercase().encode_utf16().collect();
    let needle: Vec<u16> = query.to_lowercase().encode_utf16().collect();
    let query_len = query.encode_utf16().count();
    if needle.is_empty() {
        return vec![MatchSpan {
            text: text.into(),
            inverse: false,
        }];
    }

    let mut parts = Vec::new();
    let mut offset = 0;
    while offset <= normalized.len() && needle.len() <= normalized.len() - offset {
        let Some(relative) = normalized[offset..]
            .windows(needle.len())
            .position(|window| window == needle.as_slice())
        else {
            break;
        };
        let idx = offset + relative;
        if idx > offset {
            parts.push(MatchSpan {
                text: String::from_utf16_lossy(
                    &original[offset.min(original.len())..idx.min(original.len())],
                ),
                inverse: false,
            });
        }
        parts.push(MatchSpan {
            text: String::from_utf16_lossy(
                &original[idx.min(original.len())..(idx + query_len).min(original.len())],
            ),
            inverse: true,
        });
        offset = idx + query_len;
    }
    if parts.is_empty() {
        return vec![MatchSpan {
            text: text.into(),
            inverse: false,
        }];
    }
    if offset < original.len() {
        parts.push(MatchSpan {
            text: String::from_utf16_lossy(&original[offset..]),
            inverse: false,
        });
    }
    parts
}

pub fn highlight_view(text: &str, query: &str) -> crate::generated_ports::ui::VNode {
    use crate::generated_ports::ui::VNode;
    VNode::Group(
        highlight_match(text, query)
            .into_iter()
            .map(|part| {
                if part.inverse {
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: BTreeMap::from([("inverse".into(), Value::Bool(true))]),
                        children: vec![VNode::Text(part.text)],
                    }
                } else {
                    VNode::Text(part.text)
                }
            })
            .collect(),
    )
}

// src/services/tips/tipHistory.ts
pub fn record_tip_shown(config: &mut Value, tip_id: &str) -> bool {
    let Some(obj) = config.as_object_mut() else {
        return false;
    };
    let startups = obj.get("numStartups").and_then(Value::as_i64).unwrap_or(0);
    if !obj.contains_key("tipsHistory") || !obj.get("tipsHistory").is_some_and(Value::is_object) {
        obj.insert("tipsHistory".into(), json!({}));
    }
    let hist = obj
        .get_mut("tipsHistory")
        .and_then(Value::as_object_mut)
        .unwrap();
    if hist.get(tip_id).and_then(Value::as_i64) == Some(startups) {
        return false;
    }
    hist.insert(tip_id.into(), json!(startups));
    true
}
pub fn sessions_since_tip_shown(config: &Value, tip_id: &str) -> Option<i64> {
    let startups = config
        .get("numStartups")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let seen = config.get("tipsHistory")?.get(tip_id)?.as_i64()?;
    if seen == 0 {
        None
    } else {
        Some(startups - seen)
    }
}

// src/utils/deepLink/terminalPreference.ts
pub fn terminal_preference(program: &str) -> Option<&'static str> {
    match program.to_ascii_lowercase().as_str() {
        "iterm" | "iterm.app" => Some("iTerm"),
        "ghostty" => Some("Ghostty"),
        "kitty" => Some("kitty"),
        "alacritty" => Some("Alacritty"),
        "wezterm" => Some("WezTerm"),
        "apple_terminal" => Some("Terminal"),
        _ => None,
    }
}
pub fn update_terminal_preference(config: &mut Value, program: Option<&str>, macos: bool) -> bool {
    if !macos {
        return false;
    }
    let Some(preferred) = program.and_then(terminal_preference) else {
        return false;
    };
    let Some(obj) = config.as_object_mut() else {
        return false;
    };
    if obj.get("deepLinkTerminal").and_then(Value::as_str) == Some(preferred) {
        return false;
    }
    obj.insert("deepLinkTerminal".into(), json!(preferred));
    true
}

// src/utils/userAgent.ts — original brand replaced with portable identity.
pub fn user_agent() -> String {
    format!("atlas-agent/{}", env!("CARGO_PKG_VERSION"))
}

// src/cli/exit.ts, returns a typed result so callers can exit without
// losing the error when invoked in a test or embedded SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOutcome {
    Success,
    Failure,
}
impl CliOutcome {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
        }
    }
}
pub fn print_cli_outcome(
    outcome: CliOutcome,
    message: Option<&str>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<i32> {
    if let Some(msg) = message.filter(|s| !s.is_empty()) {
        match outcome {
            CliOutcome::Success => {
                stdout.write_all(msg.as_bytes())?;
                stdout.write_all(b"\n")?
            }
            CliOutcome::Failure => {
                stderr.write_all(msg.as_bytes())?;
                stderr.write_all(b"\n")?
            }
        }
    }
    Ok(outcome.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn specs_identify_commands_without_path_traversal() {
        assert_eq!(builtin_command_specs().len(), 7);
        assert!(get_builtin_command_spec("alias").unwrap().arguments[0].variadic);
        assert!(get_builtin_command_spec("timeout").unwrap().arguments[1].is_command);
        assert!(get_builtin_command_spec("../timeout").is_none());
    }
    #[test]
    fn builtin_command_registry_reaches_full_source_srun_and_pyright_specs() {
        let srun = get_builtin_command_spec("srun").unwrap();
        assert_eq!(srun.description, "Run a command on SLURM cluster nodes");
        assert_eq!(srun.arguments.len(), 3);
        assert_eq!(srun.arguments[0].name, "-n, --ntasks <count>");
        assert_eq!(srun.arguments[1].name, "-N, --nodes <count>");
        assert_eq!(srun.arguments[2].name, "command");
        assert!(srun.arguments[2].is_command);

        let pyright = get_builtin_command_spec("pyright").unwrap();
        assert_eq!(pyright.description, "Type checker for Python");
        assert_eq!(pyright.arguments.len(), 22); (批量转写批次一：常量/插件/类型/纯函数钩子十文件（源码已提交待验收）)
        assert!(pyright
            .arguments
            .iter()
            .any(|arg| arg.name == "--threads <N>"));
        assert_eq!(pyright.arguments.last().unwrap().name, "files");
        assert!(pyright.arguments.last().unwrap().variadic);
    }

    #[test]
    fn original_direct_mention_and_highlight() {
        assert_eq!(
            parse_direct_member_message("@qa-bot  fix tests").unwrap(),
            DirectMemberMessage {
                recipient: "qa-bot".into(),
                message: "fix tests".into()
            }
        );
        assert!(parse_direct_member_message("prefix @qa fix").is_none());
        assert!(parse_direct_member_message("@a \n  ").is_none());
        let spans = highlight_match("A test a TEST", "test");
        assert_eq!(spans.iter().filter(|v| v.inverse).count(), 2);
        assert_eq!(
            spans.iter().map(|v| v.text.as_str()).collect::<String>(),
            "A test a TEST"
        );
        assert_eq!(highlight_view("hello", "ll").text_content(), "hello");
        // JavaScript indexes UTF-16 code units, not UTF-8 byte offsets.
        let spans = highlight_match("😀Hello😀", "hello");
        assert_eq!(
            spans,
            vec![
                MatchSpan {
                    text: "😀".into(),
                    inverse: false
                },
                MatchSpan {
                    text: "Hello".into(),
                    inverse: true
                },
                MatchSpan {
                    text: "😀".into(),
                    inverse: false
                },
            ]
        );
        // A lowercase expansion changes the original JavaScript slice index.
        let spans = highlight_match("İabc", "a");
        assert_eq!(
            spans,
            vec![
                MatchSpan {
                    text: "İa".into(),
                    inverse: false
                },
                MatchSpan {
                    text: "b".into(),
                    inverse: true
                },
                MatchSpan {
                    text: "c".into(),
                    inverse: false
                },
            ]
        );
    }
    #[test]
    fn history_terminal_and_cli() {
        let mut config = json!({"numStartups":7});
        assert!(record_tip_shown(&mut config, "first"));
        assert!(!record_tip_shown(&mut config, "first"));
        config["numStartups"] = json!(10);
        assert_eq!(sessions_since_tip_shown(&config, "first"), Some(3));
        assert_eq!(sessions_since_tip_shown(&config, "missing"), None);
        assert!(update_terminal_preference(
            &mut config,
            Some("iTerm.app"),
            true
        ));
        assert!(!update_terminal_preference(
            &mut config,
            Some("iTerm.app"),
            true
        ));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            print_cli_outcome(CliOutcome::Failure, Some("bad"), &mut out, &mut err).unwrap(),
            1
        );
        assert_eq!(err, b"bad\n");
        assert!(user_agent().starts_with("atlas-agent/"));
    }
}
