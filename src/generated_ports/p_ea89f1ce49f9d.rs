//! Native Rust port derived from original: src/components/StructuredDiffList.tsx
//!
//! Diff hunk list assembly: one StructuredDiff view per hunk keyed by
//! newStart, with ellipsis separators interspersed between them (not
//! after the last). The diff widget itself is component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

/// `intersperse` with ellipsis separators.
pub fn diff_list_views(hunk_keys: &[u64]) -> Vec<VNode> {
    let mut out: Vec<VNode> = Vec::new();
    for (i, key) in hunk_keys.iter().enumerate() {
        if i > 0 {
            out.push(VNode::Element {
                tag: "NoSelect".into(),
                attrs: [(
                    "fromLeftEdge".into(),
                    serde_json::json!(format!("ellipsis-{i}")),
                )]
                .into_iter()
                .collect(),
                children: vec![VNode::Text("...".into())],
            });
        }
        out.push(VNode::Element {
            tag: "StructuredDiff".into(),
            attrs: [("newStart".into(), serde_json::json!(key))]
                .into_iter()
                .collect(),
            children: vec![],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_separated_by_ellipsis_not_trailed() {
        let views = diff_list_views(&[1, 5, 9]);
        assert_eq!(views.len(), 5);
        match &views[1] {
            VNode::Element { children, .. } => {
                assert!(matches!(&children[0], VNode::Text(t) if t == "..."))
            }
            _ => panic!(),
        }
        match &views[4] {
            VNode::Element { tag, .. } => assert_eq!(tag, "StructuredDiff"),
            _ => panic!(),
        }
        assert_eq!(diff_list_views(&[]).len(), 0);
    }
}
