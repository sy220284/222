//! Context usage warning port of src/components/TokenWarning.tsx.
//! Host props are the resolved model/window, compaction settings and collapse
//! snapshot. Rendering does not silently replace live expressions with null.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};

const AUTOCOMPACT_BUFFER_TOKENS: f64 = 13_000.0;
const WARNING_BUFFER_TOKENS: f64 = 20_000.0;

fn element(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    let attrs = attrs
        .as_object()
        .expect("static view attributes")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}

fn number(props: &Value, key: &str) -> Option<f64> {
    props
        .get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
}

/// Model size must be the *effective* context window, after reserving model
/// output tokens. The host obtains this from the active provider's model.
pub fn render_source_view(props: &Value) -> VNode {
    let Some(effective_window) = number(props, "effectiveContextWindow").filter(|n| *n > 0.0)
    else {
        // No window was provided: do not invent a threshold or false alarm.
        return VNode::Empty;
    };
    let usage = number(props, "tokenUsage").unwrap_or(0.0);
    let auto = props
        .get("autoCompactEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let suppress = props
        .get("suppressWarning")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let threshold = if auto {
        effective_window - AUTOCOMPACT_BUFFER_TOKENS
    } else {
        effective_window
    };
    if threshold <= 0.0 || suppress || usage < threshold - WARNING_BUFFER_TOKENS {
        return VNode::Empty;
    }
    let percent_left = (100.0 * (threshold - usage) / threshold).round().max(0.0) as i64;
    let is_error = usage >= threshold - WARNING_BUFFER_TOKENS;
    let message = props
        .get("upgradeMessage")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let collapse = props
        .get("collapseMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if collapse {
        let stats = &props["collapseStats"];
        let collapsed = stats["collapsedSpans"].as_u64().unwrap_or(0);
        let staged = stats["stagedSpans"].as_u64().unwrap_or(0);
        let errors = stats["totalErrors"].as_u64().unwrap_or(0);
        let empty_spawns = stats["totalEmptySpawns"].as_u64().unwrap_or(0);
        let idle_warn = stats["emptySpawnWarningEmitted"].as_bool().unwrap_or(false);
        let total = collapsed.saturating_add(staged);
        let (color, label) = if errors > 0 || idle_warn {
            let problem = if errors > 0 {
                format!("collapse errors: {errors}")
            } else {
                format!("collapse idle ({empty_spawns} empty runs)")
            };
            let label = if total > 0 {
                format!("{collapsed} / {total} summarized · {problem}")
            } else {
                problem
            };
            (json!({"color":"warning","wrap":"truncate"}), label)
        } else if total > 0 {
            let label = format!("{collapsed} / {total} summarized");
            let label = if let Some(extra) = message {
                format!("{label} · {extra}")
            } else {
                label
            };
            (json!({"dimColor":true,"wrap":"truncate"}), label)
        } else {
            return VNode::Empty;
        };
        return element(
            "Box",
            json!({"flexDirection":"row"}),
            vec![element("Text", color, vec![VNode::Text(label)])],
        );
    }
    let reactive = props
        .get("reactiveOnlyMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let display_left = if reactive {
        (100.0 * (effective_window - usage) / effective_window)
            .round()
            .max(0.0) as i64
    } else {
        percent_left
    };
    let label = if auto {
        if reactive {
            format!("{}% context used", 100 - display_left)
        } else {
            format!("{display_left}% until auto-compact")
        }
    } else {
        format!("Context low ({percent_left}% remaining)")
    };
    let text = if auto {
        message
            .map(|extra| format!("{label} · {extra}"))
            .unwrap_or(label)
    } else if let Some(extra) = message {
        format!("{label} · {extra}")
    } else {
        format!("{label} · Run /compact to compact & continue")
    };
    let attrs = if auto {
        json!({"dimColor":true,"wrap":"truncate"})
    } else {
        json!({"color":if is_error {"error"} else {"warning"},"wrap":"truncate"})
    };
    element(
        "Box",
        json!({"flexDirection":"row"}),
        vec![element("Text", attrs, vec![VNode::Text(text)])],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_token_warning_when_unavailable_or_below_threshold() {
        assert_eq!(render_source_view(&json!({})), VNode::Empty);
        let props = json!({"effectiveContextWindow":100_000,"tokenUsage":20_000,"autoCompactEnabled":false});
        assert_eq!(render_source_view(&props), VNode::Empty);
    }
    #[test]
    fn displays_manual_compaction_warning_at_threshold_and_suppresses_when_requested() {
        let props = json!({"effectiveContextWindow":100_000,"tokenUsage":82_000,"autoCompactEnabled":false});
        let view = render_source_view(&props);
        assert_eq!(view.unresolved_node_count(), 0);
        assert!(view.text_content().contains("Context low (18% remaining)"));
        assert!(view.text_content().contains("/compact"));
        assert_eq!(
            render_source_view(
                &json!({"effectiveContextWindow":100_000,"tokenUsage":82_000,"suppressWarning":true})
            ),
            VNode::Empty
        );
    }
    #[test]
    fn autocompact_and_reactive_progress_keep_original_labels() {
        let base =
            json!({"effectiveContextWindow":100_000,"tokenUsage":80_000,"autoCompactEnabled":true});
        assert_eq!(
            render_source_view(&base).text_content(),
            "8% until auto-compact"
        );
        let mut reactive = base;
        reactive["reactiveOnlyMode"] = json!(true);
        assert_eq!(
            render_source_view(&reactive).text_content(),
            "80% context used"
        );
    }
    #[test]
    fn collapse_progress_and_errors_use_live_stats() {
        let props = json!({"effectiveContextWindow":100_000,"tokenUsage":85_000,"collapseMode":true,"collapseStats":{"collapsedSpans":2,"stagedSpans":1},"upgradeMessage":"upgrade available"});
        assert_eq!(
            render_source_view(&props).text_content(),
            "2 / 3 summarized · upgrade available"
        );
        let mut failed = props;
        failed["collapseStats"]["totalErrors"] = json!(1);
        assert_eq!(
            render_source_view(&failed).text_content(),
            "2 / 3 summarized · collapse errors: 1"
        );
        failed["collapseStats"] = json!({});
        assert_eq!(render_source_view(&failed), VNode::Empty);
    }
}
