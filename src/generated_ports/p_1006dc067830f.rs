//! Executable Rust view of src/components/CustomSelect/SelectMulti.tsx.
//! The input is the current selection snapshot supplied by the terminal
//! controller; event routing continues to custom_multi_select_source.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn text(content: impl Into<String>, color: Option<&str>, dim: bool) -> VNode {
    let mut attrs = BTreeMap::new();
    if let Some(c) = color {
        attrs.insert("color".into(), json!(c));
    }
    if dim {
        attrs.insert("dimColor".into(), json!(true));
    }
    VNode::Element {
        tag: "Text".into(),
        attrs,
        children: vec![VNode::Text(content.into())],
    }
}

fn element(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}

fn value_equals(a: &Value, b: &Value) -> bool {
    a == b
}

/// Derive the same visible selection tree as upstream SelectMulti from current
/// state. `focusedValue`, `selectedValues`, viewport and input map are supplied
/// by the Rust controller; initial props remain supported for source inspection.
pub fn render_source_view(props: &Value) -> VNode {
    let options: &[Value] = props["options"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let count = options.len();
    let window_size = props["visibleOptionCount"].as_u64().unwrap_or(5) as usize;
    let window_size = window_size.min(count);
    let selected = props["selectedValues"]
        .as_array()
        .or_else(|| props["defaultValue"].as_array());
    let selected: &[Value] = selected.map(Vec::as_slice).unwrap_or(&[]);
    let focus = props
        .get("focusedValue")
        .filter(|v| !v.is_null())
        .or_else(|| {
            if props["initialFocusLast"] == true {
                options.last().and_then(|item| item.get("value"))
            } else {
                options.first().and_then(|item| item.get("value"))
            }
        });
    let focused_index = focus
        .and_then(|v| {
            options
                .iter()
                .position(|opt| value_equals(&opt["value"], v))
        })
        .unwrap_or(0);
    let from = props["visibleFromIndex"]
        .as_u64()
        .map(|v| v as usize)
        .unwrap_or_else(|| focused_index.saturating_add(1).saturating_sub(window_size))
        .min(count);
    let to = props["visibleToIndex"]
        .as_u64()
        .map(|v| v as usize)
        .unwrap_or(from.saturating_add(window_size))
        .min(count);
    let from = from.min(to);
    let disabled = props["isDisabled"].as_bool().unwrap_or(false);
    let submit_focused = props["isSubmitFocused"].as_bool().unwrap_or(false);
    let hide_indexes = props["hideIndexes"].as_bool().unwrap_or(false);
    let index_width = count.to_string().len();
    let input_values = props.get("inputValues");
    let mut rows = Vec::new();
    for (idx, option) in options.iter().enumerate().take(to).skip(from) {
        let value = &option["value"];
        let focused = !disabled && focus.is_some_and(|v| value_equals(v, value)) && !submit_focused;
        let checked = selected.iter().any(|v| value_equals(v, value));
        let more_below = to < count && idx + 1 == to;
        let more_above = from > 0 && idx == from;
        let number = idx + 1;
        let mut attrs = BTreeMap::from([
            ("isFocused".into(), json!(focused)),
            ("isSelected".into(), json!(false)),
            ("shouldShowDownArrow".into(), json!(more_below)),
            ("shouldShowUpArrow".into(), json!(more_above)),
        ]);
        let marker = text(
            format!("[{}]", if checked { "✓" } else { " " }),
            checked.then_some("success"),
            false,
        );
        let content = if option["type"] == "input" {
            attrs.insert("option".into(), option.clone());
            attrs.insert("index".into(), json!(number));
            attrs.insert("maxIndexWidth".into(), json!(index_width));
            attrs.insert("layout".into(), json!("compact"));
            let key = match value {
                Value::String(v) => v.clone(),
                other => other.to_string(),
            };
            let input_value = input_values
                .and_then(|v| v.get(&key))
                .and_then(Value::as_str)
                .or_else(|| option["initialValue"].as_str())
                .unwrap_or("");
            attrs.insert("inputValue".into(), json!(input_value));
            element("SelectInputOption", attrs, vec![marker])
        } else {
            if let Some(description) = option.get("description") {
                attrs.insert("description".into(), description.clone());
            }
            let mut children = Vec::new();
            if !hide_indexes {
                children.push(text(format!("{number:>index_width$}."), None, true));
            }
            children.push(marker);
            children.push(text(
                option["label"].as_str().unwrap_or(""),
                focused.then_some("suggestion"),
                false,
            ));
            element("SelectOption", attrs, children)
        };
        rows.push(element(
            "Box",
            BTreeMap::from([("gap".into(), json!(1))]),
            vec![content],
        ));
    }
    let mut outer = vec![element(
        "Box",
        BTreeMap::from([("flexDirection".into(), json!("column"))]),
        rows,
    )];
    if let Some(label) = props["submitButtonText"].as_str().filter(|s| !s.is_empty()) {
        if props["hasOnSubmit"] == true {
            outer.push(element(
                "Box",
                BTreeMap::from([("gap".into(), json!(1)), ("marginTop".into(), json!(0))]),
                vec![
                    text(
                        if submit_focused { "❯" } else { " " },
                        submit_focused.then_some("suggestion"),
                        false,
                    ),
                    element(
                        "Box",
                        BTreeMap::from([("marginLeft".into(), json!(3))]),
                        vec![text(label, submit_focused.then_some("suggestion"), false)],
                    ),
                ],
            ));
        }
    }
    element(
        "Box",
        BTreeMap::from([("flexDirection".into(), json!("column"))]),
        outer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> Value {
        json!([
            {"value":"a","label":"Alpha","description":"first"},
            {"value":"b","label":"Beta"},
            {"value":"c","label":"Input","type":"input","initialValue":"preset"},
            {"value":"d","label":"Delta"},
            {"value":"e","label":"Echo"},
        ])
    }
    #[test]
    fn selection_window_focus_and_arrows_are_derived_from_state() {
        let v = render_source_view(&json!({"options":options(),"selectedValues":["a","d"],
            "focusedValue":"d","visibleOptionCount":2,"visibleFromIndex":2,"visibleToIndex":4}));
        assert_eq!(v.unresolved_node_count(), 0);
        let s = serde_json::to_value(v).unwrap();
        let rows = &s["children"][0]["children"];
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(rows[0]["children"][0]["tag"], "SelectInputOption");
        assert_eq!(rows[0]["children"][0]["attrs"]["shouldShowUpArrow"], true);
        assert_eq!(rows[0]["children"][0]["attrs"]["inputValue"], "preset");
        assert_eq!(rows[1]["children"][0]["attrs"]["isFocused"], true);
        assert_eq!(rows[1]["children"][0]["attrs"]["shouldShowDownArrow"], true);
    }
    #[test]
    fn hidden_indexes_and_submit_button_follow_original_conditions() {
        let v = render_source_view(&json!({"options":options(),"hideIndexes":true,
            "submitButtonText":"Save","hasOnSubmit":true,"isSubmitFocused":true,"defaultValue":["a"]}));
        assert_eq!(v.unresolved_node_count(), 0);
        let s = serde_json::to_value(v).unwrap();
        assert_eq!(s["children"].as_array().unwrap().len(), 2);
        assert_eq!(
            s["children"][1]["children"][1]["children"][0]["children"][0]["value"],
            "Save"
        );
        let children = &s["children"][0]["children"][0]["children"][0]["children"];
        assert_eq!(children.as_array().unwrap().len(), 2); // marker + title; no index
    }
    #[test]
    fn empty_options_and_disabled_focus_do_not_invent_options() {
        let v = render_source_view(&json!({"options":[],"isDisabled":true}));
        assert_eq!(v.node_count(), 2);
        assert_eq!(v.unresolved_node_count(), 0);
    }
}
