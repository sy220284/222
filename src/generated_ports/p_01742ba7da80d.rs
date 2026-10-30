//! IDE auto-connect dialogs, ported from src/components/IdeAutoConnectDialog.tsx.
//! Host-owned settings and terminal support are passed explicitly rather than
//! being replaced by a null JSX expression.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};

pub fn should_show_auto_connect_dialog(supported_terminal: bool, config: &Value) -> bool {
    !supported_terminal
        && config.get("autoConnectIde") != Some(&Value::Bool(true))
        && config.get("hasIdeAutoConnectDialogBeenShown") != Some(&Value::Bool(true))
}

pub fn should_show_disable_auto_connect_dialog(supported_terminal: bool, config: &Value) -> bool {
    !supported_terminal && config.get("autoConnectIde") == Some(&Value::Bool(true))
}

/// Source's enable choice always persists both flags and then completes.
/// The disable choice only changes persisted settings for `yes`.
pub fn apply_choice(config: &mut Value, disable_dialog: bool, answer: &str) -> bool {
    let selected_yes = answer == "yes";
    if !disable_dialog || selected_yes {
        let settings = config
            .as_object_mut()
            .expect("IDE settings must be a JSON object");
        settings.insert(
            "autoConnectIde".into(),
            Value::Bool(!disable_dialog && selected_yes),
        );
        if !disable_dialog {
            settings.insert("hasIdeAutoConnectDialogBeenShown".into(), Value::Bool(true));
        }
    }
    disable_dialog && selected_yes
}

fn node(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs: attrs
            .as_object()
            .expect("static node attributes")
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        children,
    }
}

/// Render the complete option tree for both source components. `disable`
/// selects the second export's UI, rather than a hard-coded single variant.
pub fn render_source_view(props: &Value) -> VNode {
    let disable = props.get("disable") == Some(&Value::Bool(true));
    let options = if disable {
        json!([{"label":"No","value":"no"},{"label":"Yes","value":"yes"}])
    } else {
        json!([{"label":"Yes","value":"yes"},{"label":"No","value":"no"}])
    };
    let selection = node(
        "Select",
        json!({
            "options":options,
            "defaultValue":if disable { "no" } else { "yes" },
            "onChange":"ide_auto_connect.select",
            "onCancel":if disable { "ide_auto_connect.cancel" } else { "ide_auto_connect.complete" }
        }),
        vec![],
    );
    let children = if disable {
        vec![selection]
    } else {
        vec![
            selection,
            node(
                "Text",
                json!({"dimColor":true}),
                vec![VNode::Text(
                    "You can also configure this in /config or with the --ide flag".into(),
                )],
            ),
        ]
    };
    let mut attrs = json!({
        "title":if disable { "Do you wish to disable auto-connect to IDE?" } else { "Do you wish to enable auto-connect to IDE?" },
        "color":"ide",
        "onCancel":if disable { "ide_auto_connect.cancel" } else { "ide_auto_connect.complete" }
    });
    if disable {
        attrs["subtitle"] = json!("You can also configure this in /config");
    }
    node("Dialog", attrs, children)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_predicates_use_supported_terminal_and_exact_config_flags() {
        let mut config = json!({});
        assert!(should_show_auto_connect_dialog(false, &config));
        assert!(!should_show_auto_connect_dialog(true, &config));
        assert!(!should_show_disable_auto_connect_dialog(false, &config));
        assert!(!apply_choice(&mut config, false, "no"));
        assert_eq!(config["autoConnectIde"], false);
        assert_eq!(config["hasIdeAutoConnectDialogBeenShown"], true);
        assert!(!should_show_auto_connect_dialog(false, &config));
        let mut enabled = json!({"unrelated":42});
        assert!(!apply_choice(&mut enabled, false, "yes"));
        assert_eq!(enabled["autoConnectIde"], true);
        assert_eq!(enabled["unrelated"], 42);
        assert!(should_show_disable_auto_connect_dialog(false, &enabled));
        assert!(!should_show_disable_auto_connect_dialog(true, &enabled));
        assert!(!apply_choice(&mut enabled, true, "no"));
        assert_eq!(enabled["autoConnectIde"], true);
        assert!(apply_choice(&mut enabled, true, "yes"));
        assert_eq!(enabled["autoConnectIde"], false);
    }
    #[test]
    fn both_source_dialogs_render_real_select_options_and_cancel_actions() {
        let enabled = serde_json::to_value(render_source_view(&json!({}))).unwrap();
        assert_eq!(
            enabled["attrs"]["title"],
            "Do you wish to enable auto-connect to IDE?"
        );
        assert_eq!(enabled["children"][0]["attrs"]["defaultValue"], "yes");
        assert_eq!(
            enabled["children"][0]["attrs"]["options"][0]["value"],
            "yes"
        );
        assert_eq!(enabled["children"][1]["tag"], "Text");
        let disabled = serde_json::to_value(render_source_view(&json!({"disable":true}))).unwrap();
        assert_eq!(
            disabled["attrs"]["title"],
            "Do you wish to disable auto-connect to IDE?"
        );
        assert_eq!(
            disabled["children"][0]["attrs"]["options"][0]["value"],
            "no"
        );
        assert_eq!(disabled["children"][0]["attrs"]["defaultValue"], "no");
        assert_eq!(disabled["attrs"]["onCancel"], "ide_auto_connect.cancel");
        assert_eq!(render_source_view(&json!({})).unresolved_node_count(), 0);
    }
}
