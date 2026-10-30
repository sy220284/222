//! Source: src/components/FilePathLink.tsx.
//! Render an OSC 8 terminal link with Node pathToFileURL-compatible escaping.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};

pub fn render_source_view(props: &Value) -> VNode {
    let Some(file_path) = props.get("filePath").and_then(Value::as_str) else {
        return VNode::Empty;
    };
    // pathToFileURL resolves paths relative to cwd before percent-encoding.
    let path = PathBuf::from(file_path);
    let absolute = if path.is_absolute() {
        path
    } else {
        let Ok(cwd) = std::env::current_dir() else {
            return VNode::Empty;
        };
        cwd.join(path)
    };
    let Ok(uri) = reqwest::Url::from_file_path(absolute) else {
        return VNode::Empty;
    };
    let display = match props.get("children") {
        Some(Value::Null) | None => VNode::Text(file_path.into()),
        Some(Value::String(text)) => VNode::Text(text.clone()),
        Some(value) => serde_json::from_value::<VNode>(value.clone())
            .unwrap_or_else(|_| VNode::Value(value.clone())),
    };
    VNode::Element {
        tag: "Link".into(),
        attrs: BTreeMap::from([("url".into(), json!(uri.as_str()))]),
        children: vec![display],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_link_encodes_special_characters_and_defaults_to_path_text() {
        let input = "/tmp/a b#100%中文.txt";
        let node = render_source_view(&json!({"filePath":input}));
        assert_eq!(node.text_content(), input);
        assert_eq!(node.unresolved_node_count(), 0);
        let VNode::Element { tag, attrs, .. } = node else {
            panic!("expected Link")
        };
        assert_eq!(tag, "Link");
        let url = attrs.get("url").and_then(Value::as_str).unwrap();
        assert!(url.starts_with("file:///"), "{url}");
        assert!(
            url.ends_with("/tmp/a%20b%23100%25%E4%B8%AD%E6%96%87.txt"),
            "{url}"
        );
    }

    #[test]
    fn explicit_child_text_and_rich_node_are_preserved() {
        let node = render_source_view(&json!({"filePath":"/tmp/a.txt","children":"read file"}));
        assert_eq!(node.text_content(), "read file");
        let node = render_source_view(&json!({
            "filePath":"/tmp/a.txt",
            "children":{"kind":"element","tag":"Text","attrs":{"bold":true},"children":[{"kind":"text","value":"A"}]}
        }));
        assert_eq!(node.text_content(), "A");
        assert_eq!(node.unresolved_node_count(), 0);
        assert_eq!(render_source_view(&json!({})), VNode::Empty);
    }
}
