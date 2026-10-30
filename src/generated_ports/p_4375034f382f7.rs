//! Manual source-order view for `src/components/PromptInput/Notifications.tsx`.
//! All reactive state and elapsed time are explicit host inputs. Provider
//! authentication diagnostics are vendor-neutral in this Rust application.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn FOOTER_TEMPORARY_STATUS_TIMEOUT() -> Value {
    json!(5000)
}
fn node(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}
fn attrs(props: &Value, names: &[&str]) -> BTreeMap<String, Value> {
    names
        .iter()
        .filter_map(|key| props.get(*key).map(|value| ((*key).into(), value.clone())))
        .collect()
}
fn bool_prop(props: &Value, key: &str) -> bool {
    props.get(key).and_then(Value::as_bool).unwrap_or(false)
}
fn text(content: &str, color: Option<&str>, dim: bool) -> VNode {
    let mut attrs = BTreeMap::from([
        ("dimColor".into(), json!(dim)),
        ("wrap".into(), json!("truncate")),
    ]);
    if let Some(color) = color {
        attrs.insert("color".into(), json!(color));
    }
    node("Text", attrs, vec![VNode::Text(content.into())])
}
fn box_text(content: &str, color: Option<&str>, dim: bool) -> VNode {
    node("Box", BTreeMap::new(), vec![text(content, color, dim)])
}
/// The source's hint is an effect, not a permanent line. The host presents
/// this content as a timed notification only when eligible.
pub fn external_editor_hint(props: &Value) -> Option<Value> {
    let auth_ok = !matches!(
        props.get("apiKeyStatus").and_then(Value::as_str),
        Some("missing" | "invalid")
    );
    let show = bool_prop(props, "isInputWrapped")
        && !bool_prop(props, "isShowingCompactMessage")
        && auth_ok
        && props
            .get("externalEditor")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
    if !show {
        return None;
    }
    let editor = props.get("externalEditor")?.as_str()?;
    Some(
        json!({"key":"external-editor-hint","priority":"immediate","timeoutMs":5000,
       "action":"chat:externalEditor","shortcut":"ctrl+g","description":format!("edit in {editor}")}),
    )
}
fn notification_content(props: &Value) -> VNode {
    let voice_active = bool_prop(props, "voiceFeatureEnabled") && bool_prop(props, "voiceEnabled");
    let voice_state = props
        .get("voiceState")
        .and_then(Value::as_str)
        .unwrap_or("idle");
    if voice_active && matches!(voice_state, "recording" | "processing") {
        return node(
            "VoiceIndicator",
            BTreeMap::from([("voiceState".into(), json!(voice_state))]),
            vec![],
        );
    }
    let mut children = vec![node(
        "IdeStatusIndicator",
        attrs(props, &["ideSelection", "mcpClients"]),
        vec![],
    )];
    if let Some(notification) = props.get("notifications").and_then(|n| n.get("current")) {
        if !notification.is_null() {
            let mut notice_attrs = attrs(notification, &["color", "key"]);
            notice_attrs.insert("wrap".into(), json!("truncate"));
            if notification.get("color").is_none() {
                notice_attrs.insert("dimColor".into(), json!(true));
            }
            let content = if let Some(jsx) = notification.get("jsx") {
                VNode::Value(jsx.clone())
            } else if let Some(text) = notification.get("text").and_then(Value::as_str) {
                VNode::Text(text.into())
            } else {
                VNode::Empty
            };
            children.push(node("Text", notice_attrs, vec![content]));
        }
    }
    if bool_prop(props, "isInOverageMode")
        && !matches!(
            props.get("subscriptionType").and_then(Value::as_str),
            Some("team" | "enterprise")
        )
    {
        children.push(box_text("Now using extra usage", None, true));
    }
    if let Some(elapsed) = props.get("apiKeyHelperElapsedMs").and_then(Value::as_u64) {
        if elapsed >= 10_000 && bool_prop(props, "apiKeyHelperConfigured") {
            let seconds = elapsed.div_ceil(1000);
            children.push(node(
                "Box",
                BTreeMap::new(),
                vec![
                    text("apiKeyHelper is taking a while ", Some("warning"), false),
                    text(&format!("({seconds}s)"), None, true),
                ],
            ));
        }
    }
    if matches!(
        props.get("apiKeyStatus").and_then(Value::as_str),
        Some("invalid" | "missing")
    ) {
        // Provider-neutral replacement for source's vendor-specific login text.
        children.push(box_text(
            "Model provider authentication failed · Check model settings",
            Some("error"),
            false,
        ));
    }
    if bool_prop(props, "debug") {
        children.push(box_text("Debug mode", Some("warning"), false));
    }
    if !matches!(
        props.get("apiKeyStatus").and_then(Value::as_str),
        Some("invalid" | "missing")
    ) && bool_prop(props, "verbose")
    {
        let count = props.get("tokenUsage").and_then(Value::as_u64).unwrap_or(0);
        children.push(box_text(&format!("{count} tokens"), None, true));
    }
    if !bool_prop(props, "isBriefOnly") {
        children.push(node(
            "TokenWarning",
            attrs(props, &["tokenUsage", "mainLoopModel"]),
            vec![],
        ));
    }
    if bool_prop(props, "shouldShowAutoUpdater") {
        let mut params = attrs(props, &["verbose", "autoUpdaterResult", "isAutoUpdating"]);
        params.insert(
            "showSuccessMessage".into(),
            json!(!bool_prop(props, "isShowingCompactMessage")),
        );
        children.push(node("AutoUpdaterWrapper", params, vec![]));
    }
    if voice_active {
        if let Some(error) = props
            .get("voiceError")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            children.push(box_text(error, Some("error"), false));
        }
    }
    children.push(node("MemoryUsageIndicator", BTreeMap::new(), vec![]));
    children.push(node("SandboxPromptFooterHint", BTreeMap::new(), vec![]));
    VNode::Group(children)
}
pub fn render_source_view(props: &Value) -> VNode {
    let selection = props.get("ideSelection");
    let is_selected = selection.is_some_and(|s| {
        s.get("filePath")
            .and_then(Value::as_str)
            .is_some_and(|t| !t.is_empty())
            || s.get("text")
                .and_then(Value::as_str)
                .is_some_and(|t| !t.is_empty())
                && s.get("lineCount").and_then(Value::as_u64).unwrap_or(0) > 0
    });
    let should_show_selection =
        props.get("ideStatus").and_then(Value::as_str) == Some("connected") && is_selected;
    let update_status = props
        .get("autoUpdaterResult")
        .and_then(|x| x.get("status"))
        .and_then(Value::as_str);
    let show_updater = !should_show_selection
        || bool_prop(props, "isAutoUpdating")
        || update_status != Some("success");
    let mut prepared = props.clone();
    // The React component calculates this from its hook inputs. This native
    // inspector requires a fully resolved token/model warning state in props.
    if let Some(map) = prepared.as_object_mut() {
        map.insert("shouldShowAutoUpdater".into(), json!(show_updater));
    }
    let content = notification_content(&prepared);
    node(
        "SentryErrorBoundary",
        BTreeMap::new(),
        vec![node(
            "Box",
            BTreeMap::from([
                ("flexDirection".into(), json!("column")),
                (
                    "alignItems".into(),
                    json!(if bool_prop(props, "isNarrow") {
                        "flex-start"
                    } else {
                        "flex-end"
                    }),
                ),
                ("flexShrink".into(), json!(0)),
                ("overflowX".into(), json!("hidden")),
            ]),
            vec![content],
        )],
    )
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn recording_voice_preempts_all_notifications() {
        let view = notification_content(&json!({"voiceFeatureEnabled":true,"voiceEnabled":true,
          "voiceState":"recording","apiKeyStatus":"invalid","debug":true}));
        let VNode::Element { tag, .. } = view else {
            panic!("voice")
        };
        assert_eq!(tag, "VoiceIndicator");
    }
    #[test]
    fn ide_selection_suppresses_successful_updater_without_hiding_errors() {
        let mut props = json!({"ideStatus":"connected","ideSelection":{"filePath":"hello.rs"},
            "autoUpdaterResult":{"status":"success"},"isAutoUpdating":false,"isNarrow":true});
        let view = render_source_view(&props);
        let VNode::Element { children, .. } = view else {
            panic!("error boundary")
        };
        assert!(!children[0]
            .text_content()
            .contains("Model provider authentication failed"));
        let VNode::Element {
            attrs, children, ..
        } = &children[0]
        else {
            panic!("box")
        };
        assert_eq!(attrs["alignItems"], "flex-start");
        let VNode::Group(items) = &children[0] else {
            panic!("notifications")
        };
        assert!(!items
            .iter()
            .any(|item| matches!(item,VNode::Element{tag,..} if tag=="AutoUpdaterWrapper")));
        props["isAutoUpdating"] = json!(true);
        let VNode::Element { children, .. } = render_source_view(&props) else {
            panic!("boundary")
        };
        let VNode::Element { children, .. } = &children[0] else {
            panic!("box")
        };
        let VNode::Group(items) = &children[0] else {
            panic!("notifications")
        };
        assert!(items
            .iter()
            .any(|item| matches!(item,VNode::Element{tag,..} if tag=="AutoUpdaterWrapper")));
    }
    #[test]
    fn provider_failure_overrides_verbose_usage_and_editor_hint() {
        let p = json!({"apiKeyStatus":"invalid","verbose":true,"externalEditor":"vim","isInputWrapped":true});
        assert!(external_editor_hint(&p).is_none());
        let view = notification_content(&p);
        assert!(view
            .text_content()
            .contains("Model provider authentication failed"));
        assert!(!view.text_content().contains("tokens"));
    }
}
