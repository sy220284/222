//! Native Rust UI structure derived from original: src/components/HistorySearchDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "FuzzyPicker".into(),
        attrs: vec![
            ("title".into(), Value::String("Search prompts".into())),
            (
                "placeholder".into(),
                Value::String("Filter history…".into()),
            ),
            ("initialQuery".into(), Value::Null),
            ("items".into(), Value::Null),
            ("getKey".into(), Value::Null),
            ("onQueryChange".into(), Value::Null),
            ("onSelect".into(), Value::Null),
            ("onCancel".into(), Value::Null),
            ("emptyMessage".into(), Value::Null),
            ("selectAction".into(), Value::String("use".into())),
            ("direction".into(), Value::String("up".into())),
            (
                "previewPosition".into(),
                if crate::generated_ports::runtime::truthy(&Value::Null) {
                    Value::String("right".into())
                } else {
                    Value::String("bottom".into())
                },
            ),
            ("renderItem".into(), Value::Null),
            ("renderPreview".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
