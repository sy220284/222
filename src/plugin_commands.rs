//! Complete subcommand parser for 2.1.88 src/commands/plugin/parseArgs.ts.
//! Parsed commands are shared by the native plugin manager and the CLI.
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PluginCommand {
    Menu,
    Help,
    Install {
        #[serde(skip_serializing_if = "Option::is_none")]
        marketplace: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin: Option<String>,
    },
    Manage,
    Uninstall {
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin: Option<String>,
    },
    Enable {
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin: Option<String>,
    },
    Disable {
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin: Option<String>,
    },
    Validate {
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Marketplace {
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
}

pub fn parse_plugin_args(args: Option<&str>) -> PluginCommand {
    let Some(args) = args else {
        return PluginCommand::Menu;
    };
    let parts: Vec<_> = args.split_whitespace().collect();
    let command = parts.first().map(|part| part.to_ascii_lowercase());
    let target = parts.get(1).map(|part| (*part).to_owned());
    match command.as_deref() {
        Some("help" | "--help" | "-h") => PluginCommand::Help,
        Some("install" | "i") => {
            let Some(target) = target else {
                return PluginCommand::Install {
                    marketplace: None,
                    plugin: None,
                };
            };
            if target.contains('@') {
                let mut pair = target.split('@');
                return PluginCommand::Install {
                    plugin: pair.next().map(str::to_owned),
                    marketplace: pair.next().map(str::to_owned),
                };
            }
            let marketplace = target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("file://")
                || target.contains('/')
                || target.contains('\\');
            if marketplace {
                PluginCommand::Install {
                    marketplace: Some(target),
                    plugin: None,
                }
            } else {
                PluginCommand::Install {
                    marketplace: None,
                    plugin: Some(target),
                }
            }
        }
        Some("manage") => PluginCommand::Manage,
        Some("uninstall") => PluginCommand::Uninstall { plugin: target },
        Some("enable") => PluginCommand::Enable { plugin: target },
        Some("disable") => PluginCommand::Disable { plugin: target },
        Some("validate") => {
            let joined = parts.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
            PluginCommand::Validate {
                path: (!joined.trim().is_empty()).then_some(joined.trim().to_owned()),
            }
        }
        Some("marketplace" | "market") => {
            let action = parts.get(1).map(|part| part.to_ascii_lowercase());
            match action.as_deref() {
                Some("add" | "remove" | "rm" | "update" | "list") => {
                    let action = match action.as_deref() {
                        Some("rm") => "remove",
                        Some(other) => other,
                        None => unreachable!(),
                    };
                    PluginCommand::Marketplace {
                        action: Some(action.into()),
                        target: (action != "list")
                            .then(|| parts.iter().skip(2).copied().collect::<Vec<_>>().join(" ")),
                    }
                }
                _ => PluginCommand::Marketplace {
                    action: None,
                    target: None,
                },
            }
        }
        _ => PluginCommand::Menu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_and_empty_input_match_source() {
        assert_eq!(parse_plugin_args(None), PluginCommand::Menu);
        assert_eq!(parse_plugin_args(Some("")), PluginCommand::Menu);
        assert_eq!(parse_plugin_args(Some("unknown")), PluginCommand::Menu);
        assert_eq!(parse_plugin_args(Some("  -H  ")), PluginCommand::Help);
        assert_eq!(parse_plugin_args(Some("manage")), PluginCommand::Manage);
        assert_eq!(
            parse_plugin_args(Some("i linter@verified")),
            PluginCommand::Install {
                plugin: Some("linter".into()),
                marketplace: Some("verified".into()),
            }
        );
        assert_eq!(
            parse_plugin_args(Some("install https://example.com/market.json")),
            PluginCommand::Install {
                marketplace: Some("https://example.com/market.json".into()),
                plugin: None,
            }
        );
        assert_eq!(
            parse_plugin_args(Some("install plugin-name")),
            PluginCommand::Install {
                plugin: Some("plugin-name".into()),
                marketplace: None,
            }
        );
    }

    #[test]
    fn marketplace_and_validation_preserve_multiword_arguments() {
        assert_eq!(
            parse_plugin_args(Some("validate /home/me/my plugin")),
            PluginCommand::Validate {
                path: Some("/home/me/my plugin".into())
            }
        );
        assert_eq!(
            parse_plugin_args(Some("market rm old marketplace")),
            PluginCommand::Marketplace {
                action: Some("remove".into()),
                target: Some("old marketplace".into()),
            }
        );
        assert_eq!(
            parse_plugin_args(Some("marketplace list")),
            PluginCommand::Marketplace {
                action: Some("list".into()),
                target: None,
            }
        );
        assert_eq!(
            parse_plugin_args(Some("marketplace mystery")),
            PluginCommand::Marketplace {
                action: None,
                target: None
            }
        );
        assert_eq!(
            parse_plugin_args(Some("enable myplugin")),
            PluginCommand::Enable {
                plugin: Some("myplugin".into())
            }
        );
    }
}
