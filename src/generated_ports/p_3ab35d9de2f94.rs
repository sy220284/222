//! Source-aligned idle-return dialog from src/components/IdleReturnDialog.tsx.
//! The host must invoke `onDone` using the action represented by a selected
//! option. Cancel always resolves to `dismiss`, rather than starting a session.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};

pub const ACTION_CONTINUE: &str = "continue";
pub const ACTION_CLEAR: &str = "clear";
pub const ACTION_DISMISS: &str = "dismiss";
pub const ACTION_NEVER: &str = "never";

pub fn validate_action(action: &str) -> Option<&'static str> {
    match action {
        ACTION_CONTINUE => Some(ACTION_CONTINUE),
        ACTION_CLEAR => Some(ACTION_CLEAR),
        ACTION_DISMISS => Some(ACTION_DISMISS),
        ACTION_NEVER => Some(ACTION_NEVER),
        _ => None,
    }
}

pub fn format_idle_duration(minutes: f64) -> String {
    if minutes < 1.0 {
        return "< 1m".to_string();
    }
    if minutes < 60.0 {
        return format!("{}m", minutes.floor() as u64);
    }
    let hours = (minutes / 60.0).floor() as u64;
    let remainder = (minutes % 60.0).floor() as u64;
    if remainder == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {remainder}m")
    }
}

// Original `formatTokens` delegates to compact en-US Intl.NumberFormat
// (one fractional digit for >= 1000), lowercases the suffix and drops .0.
pub fn format_token_count(count: f64) -> String {
    if count < 1_000.0 {
        return format!("{:.0}", count);
    }
    let (scale, suffix) = if count >= 1_000_000_000_000.0 {
        (1_000_000_000_000.0, "t")
    } else if count >= 1_000_000_000.0 {
        (1_000_000_000.0, "b")
    } else if count >= 1_000_000.0 {
        (1_000_000.0, "m")
    } else {
        (1_000.0, "k")
    };
    let rounded = (count / scale * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}{suffix}")
    } else {
        format!("{rounded:.1}{suffix}")
    }
}

fn element(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    let attrs = attrs
        .as_object()
        .expect("static dialog attributes")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}

/// Render a user-visible selection and its observable action contract.
/// The options are the exact four-way protocol from the original dialog:
/// three visible choices, with dismiss reserved for Escape/cancel.
pub fn render_source_view(props: &Value) -> VNode {
    let (Some(idle_minutes), Some(total_tokens)) = (
        props.get("idleMinutes").and_then(Value::as_f64),
        props.get("totalInputTokens").and_then(Value::as_f64),
    ) else {
        return VNode::Empty;
    };
    if !idle_minutes.is_finite() || !total_tokens.is_finite() || total_tokens < 0.0 {
        return VNode::Empty;
    }

    let title = format!(
        "You've been away {} and this conversation is {} tokens.",
        format_idle_duration(idle_minutes),
        format_token_count(total_tokens),
    );
    element(
        "Dialog",
        json!({"title":title,"onCancel": ACTION_DISMISS}),
        vec![
            element(
                "Box",
                json!({"flexDirection":"column"}),
                vec![element(
                    "Text",
                    json!({}),
                    vec![VNode::Text(
                        "If this is a new task, clearing context will save usage and be faster."
                            .into(),
                    )],
                )],
            ),
            element(
                "Select",
                json!({
                    "options": [
                        {"value": ACTION_CONTINUE, "label": "Continue this conversation"},
                        {"value": ACTION_CLEAR, "label": "Send message as a new conversation"},
                        {"value": ACTION_NEVER, "label": "Don't ask me again"},
                    ],
                    "onChange": "onDone",
                }),
                vec![],
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_idle_duration_boundaries() {
        for (minutes, expected) in [
            (0.5, "< 1m"),
            (1.9, "1m"),
            (59.99, "59m"),
            (60.0, "1h"),
            (61.9, "1h 1m"),
            (125.0, "2h 5m"),
        ] {
            assert_eq!(format_idle_duration(minutes), expected);
        }
    }

    #[test]
    fn compact_tokens_follow_source_number_formatter() {
        for (count, expected) in [
            (900.0, "900"),
            (1_000.0, "1k"),
            (1_250.0, "1.3k"),
            (12_500.0, "12.5k"),
            (1_000_000.0, "1m"),
        ] {
            assert_eq!(format_token_count(count), expected);
        }
    }

    #[test]
    fn options_cancel_and_host_callbacks_match_source() {
        let view = render_source_view(&json!({
            "idleMinutes": 61,
            "totalInputTokens": 12_500,
        }));
        let VNode::Element {
            tag,
            attrs,
            children,
        } = view
        else {
            panic!("expected a dialog")
        };
        assert_eq!(tag, "Dialog");
        assert_eq!(attrs["onCancel"], ACTION_DISMISS);
        assert_eq!(
            attrs["title"],
            "You've been away 1h 1m and this conversation is 12.5k tokens."
        );
        let VNode::Element { tag, attrs, .. } = &children[1] else {
            panic!("expected the choice selector")
        };
        assert_eq!(tag, "Select");
        assert_eq!(attrs["onChange"], "onDone");
        assert_eq!(
            attrs["options"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["value"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![ACTION_CONTINUE, ACTION_CLEAR, ACTION_NEVER]
        );
        for action in [ACTION_CONTINUE, ACTION_CLEAR, ACTION_DISMISS, ACTION_NEVER] {
            assert_eq!(validate_action(action), Some(action));
        }
        assert_eq!(validate_action("unsafe"), None);
        assert_eq!(render_source_view(&json!({})), VNode::Empty);
    }
}
