//! Original: src/components/NotebookEditToolUseRejectedMessage.tsx.
//! Preserve the rejected operation, notebook path, cell id and code preview.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

fn el(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs: attrs
            .as_object()
            .expect("static attributes")
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        children,
    }
}

fn relative_path(cwd: &Path, path: &Path) -> String {
    // Source props may contain POSIX workspace paths even on a Windows host.
    // Resolve their lexical slash components instead of treating '/repo' as
    // a Windows drive-relative path.
    if let (Some(root), Some(target)) = (cwd.to_str(), path.to_str()) {
        if root.starts_with('/') && target.starts_with('/') {
            let root_parts = root
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            let target_parts = target
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            let shared = root_parts
                .iter()
                .zip(&target_parts)
                .take_while(|(a, b)| a == b)
                .count();
            if shared == 0 {
                return target.to_owned();
            }
            return std::iter::repeat("..")
                .take(root_parts.len() - shared)
                .chain(target_parts[shared..].iter().copied())
                .collect::<Vec<_>>()
                .join("/");
        }
    }
    if !path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    let root: Vec<Component<'_>> = cwd.components().collect();
    let dest: Vec<Component<'_>> = path.components().collect();
    let shared = root
        .iter()
        .zip(dest.iter())
        .take_while(|(a, b)| a == b)
        .count();
    if shared == 0 {
        return path.to_string_lossy().into_owned();
    }
    let mut relative = std::path::PathBuf::new();
    for component in &root[shared..] {
        if !matches!(component, Component::CurDir) {
            relative.push("..");
        }
    }
    for component in &dest[shared..] {
        relative.push(component.as_os_str());
    }
    relative.to_string_lossy().into_owned()
}

pub fn render_source_view(props: &Value) -> VNode {
    let Some(path) = props.get("notebook_path").and_then(Value::as_str) else {
        return VNode::Empty;
    };
    let Some(new_source) = props.get("new_source").and_then(Value::as_str) else {
        return VNode::Empty;
    };
    let Some(verbose) = props.get("verbose").and_then(Value::as_bool) else {
        return VNode::Empty;
    };
    let mode = props
        .get("edit_mode")
        .and_then(Value::as_str)
        .unwrap_or("replace");
    let operation = if mode == "delete" {
        "delete".to_owned()
    } else {
        format!("{mode} cell in")
    };
    let cwd = props
        .get("cwd")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let shown_path = if verbose {
        path.to_owned()
    } else if let Some(cwd) = cwd {
        relative_path(&cwd, Path::new(path))
    } else {
        path.to_owned()
    };
    let cell = props.get("cell_id").and_then(Value::as_str).unwrap_or("");
    let header = el(
        "Box",
        json!({"flexDirection":"row"}),
        vec![
            el(
                "Text",
                json!({"color":"subtle"}),
                vec![VNode::Text(format!("User rejected {operation} "))],
            ),
            el(
                "Text",
                json!({"bold":true,"color":"subtle"}),
                vec![VNode::Text(shown_path)],
            ),
            el(
                "Text",
                json!({"color":"subtle"}),
                vec![VNode::Text(format!(" at cell {cell}"))],
            ),
        ],
    );
    let mut children = vec![header];
    if mode != "delete" {
        children.push(el("Box",json!({"marginTop":1,"flexDirection":"column"}),vec![
            el("HighlightedCode",json!({"code":new_source,"filePath":if props.get("cell_type").and_then(Value::as_str)==Some("markdown") {"file.md"} else {"file.py"},"dim":true}),vec![])
        ]));
    }
    el(
        "MessageResponse",
        json!({}),
        vec![el("Box", json!({"flexDirection":"column"}), children)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_replace_shows_relative_path_cell_and_source() {
        let node = render_source_view(
            &json!({"notebook_path":"/repo/notebooks/analysis.ipynb","new_source":"print(1)","cell_id":"cell42","cell_type":"code","verbose":false,"cwd":"/repo"}),
        );
        assert_eq!(node.unresolved_node_count(), 0);
        assert!(node
            .text_content()
            .contains("User rejected replace cell in notebooks/analysis.ipynb at cell cell42"));
        let VNode::Element { children, .. } = &node else {
            panic!("response")
        };
        let VNode::Element { children, .. } = &children[0] else {
            panic!("layout")
        };
        assert_eq!(children.len(), 2);
        let VNode::Element { children, .. } = &children[1] else {
            panic!("source box")
        };
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("syntax preview")
        };
        assert_eq!(attrs.get("code"), Some(&json!("print(1)")));
        assert_eq!(attrs.get("filePath"), Some(&json!("file.py")));
    }
    #[test]
    fn delete_hides_source_and_verbose_keeps_full_path() {
        let node = render_source_view(
            &json!({"notebook_path":"/repo/a.ipynb","new_source":"x","edit_mode":"delete","verbose":true,"cwd":"/repo"}),
        );
        assert_eq!(
            node.text_content(),
            "User rejected delete /repo/a.ipynb at cell "
        );
        let VNode::Element { children, .. } = &node else {
            panic!("response")
        };
        let VNode::Element { children, .. } = &children[0] else {
            panic!("layout")
        };
        assert_eq!(children.len(), 1);
        assert_eq!(node.unresolved_node_count(), 0);
    }
    #[test]
    fn insert_markdown_and_parent_relative_paths() {
        let node = render_source_view(
            &json!({"notebook_path":"/other/a.ipynb","new_source":"# hi","edit_mode":"insert","cell_type":"markdown","verbose":false,"cwd":"/repo/sub"}),
        );
        assert!(node.text_content().contains("../../other/a.ipynb"));
        let VNode::Element { children, .. } = &node else {
            panic!("response")
        };
        let VNode::Element { children, .. } = &children[0] else {
            panic!("layout")
        };
        let VNode::Element { children, .. } = &children[1] else {
            panic!("source box")
        };
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("source code")
        };
        assert_eq!(attrs.get("filePath"), Some(&json!("file.md")));
    }
}
