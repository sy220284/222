//! Rust exports derived from original: src/ink/squash-text-nodes.ts
//!
//! Full equivalence: squash text nodes into styled segments, propagating
//! styles and the nearest hyperlink down the tree. The Rust ink DOM uses a
//! simplified node shape mirroring DOMElement/TextStyles from the ink port.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyles {
    pub dim: bool,
    pub bold: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledSegment {
    pub text: String,
    pub styles: TextStyles,
    pub hyperlink: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DomNode {
    Text(String),
    Element {
        text_styles: Option<TextStyles>,
        hyperlink: Option<String>,
        children: Vec<DomNode>,
    },
}

/// `squashTextNodesToSegments`.
pub fn squash_text_nodes_to_segments(
    node: &DomNode,
    inherited: &TextStyles,
    inherited_hyperlink: Option<&str>,
    out: &mut Vec<StyledSegment>,
) {
    match node {
        DomNode::Text(text) => out.push(StyledSegment {
            text: text.clone(),
            styles: inherited.clone(),
            hyperlink: inherited_hyperlink.map(str::to_owned),
        }),
        DomNode::Element {
            text_styles,
            hyperlink,
            children,
        } => {
            let merged = TextStyles {
                dim: text_styles.as_ref().map(|s| s.dim).unwrap_or(inherited.dim),
                bold: text_styles
                    .as_ref()
                    .map(|s| s.bold)
                    .unwrap_or(inherited.bold),
            };
            let link = hyperlink.as_deref().or(inherited_hyperlink);
            for child in children {
                squash_text_nodes_to_segments(child, &merged, link, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propagation_and_inheritance() {
        let tree = DomNode::Element {
            text_styles: Some(TextStyles {
                dim: true,
                bold: false,
            }),
            hyperlink: Some("https://example.test".into()),
            children: vec![
                DomNode::Text("a".into()),
                DomNode::Element {
                    text_styles: Some(TextStyles {
                        dim: false,
                        bold: true,
                    }),
                    hyperlink: None,
                    children: vec![DomNode::Text("b".into())],
                },
            ],
        };
        let mut out = Vec::new();
        squash_text_nodes_to_segments(&tree, &TextStyles::default(), None, &mut out);
        assert_eq!(out.len(), 2);
        assert!(out[0].styles.dim && !out[0].styles.bold);
        assert_eq!(out[0].hyperlink.as_deref(), Some("https://example.test"));
        // Inner element overrides dim back to false, keeps inherited link.
        assert!(!out[1].styles.dim && out[1].styles.bold);
        assert_eq!(out[1].hyperlink.as_deref(), Some("https://example.test"));
    }
}
