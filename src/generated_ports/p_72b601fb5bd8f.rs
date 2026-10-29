//! Native Rust UI structure derived from original: src/components/QuickOpenDialog.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"FuzzyPicker".into(),attrs:vec![("title".into(),Value::String("Quick Open".into())),("placeholder".into(),Value::String("Type to search files…".into())),("items".into(),Value::Null),("getKey".into(),Value::Null),("visibleCount".into(),Value::Null),("direction".into(),Value::String("up".into())),("previewPosition".into(),Value::Null),("onQueryChange".into(),Value::Null),("onFocus".into(),Value::Null),("onSelect".into(),Value::Null),("onTab".into(),Value::Null),("onShiftTab".into(),Value::Null),("onCancel".into(),Value::Null),("emptyMessage".into(),Value::Null),("selectAction".into(),Value::String("open in editor".into())),("renderItem".into(),Value::Null),("renderPreview".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
