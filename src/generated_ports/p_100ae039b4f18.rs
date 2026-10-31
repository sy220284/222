//! Native Rust port derived from original: src/commands/plugin/AddMarketplace.tsx
//!
//! Marketplace-add orchestration. Error-message table and flow are
//! verbatim: empty input, unparseable input (with the three accepted
//! formats), parsed-as-error, then the success chain (add -> save to
//! settings -> clear caches -> telemetry -> onAddComplete -> CLI result
//! or view switch). The auto-add effect fires once per mount. Network
//! and settings services are injected as plain results; text input and
//! spinner are component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

pub const MSG_EMPTY_INPUT: &str = "Please enter a marketplace source";
pub const MSG_INVALID_FORMAT: &str =
    "Invalid marketplace source format. Try: owner/repo, https://..., or ./path";
pub const MSG_SUCCESS_PREFIX: &str = "Successfully added marketplace: ";

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    pub source: String,
    /// Present for github-type sources (owner/repo).
    pub repo: Option<String>,
}

/// `parseMarketplaceInput` port stub: accepts owner/repo, https URL,
/// and ./path shapes; error maps to the invalid-format message. The
/// real network resolution lives with the marketplaceManager port.
pub fn parse_marketplace_input(input: &str) -> Result<ParsedSource, String> {
    let s = input.trim();
    if let Some(rest) = s.strip_prefix("./") {
        if !rest.is_empty() && !rest.contains(' ') {
            return Ok(ParsedSource {
                source: "./path".into(),
                repo: None,
            });
        }
        return Err(MSG_INVALID_FORMAT.into());
    }
    if s.starts_with("https://") {
        return Ok(ParsedSource {
            source: "github".into(),
            repo: None,
        });
    }
    match s.split_once('/') {
        Some((o, r)) if !o.is_empty() && !r.is_empty() && !r.contains(' ') && !r.contains("..") => {
            Ok(ParsedSource {
                source: "github".into(),
                repo: Some(s.to_string()),
            })
        }
        _ => Err(MSG_INVALID_FORMAT.into()),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddOutcome {
    Error(String),
    Success { name: String, cli_result: String },
}

/// `handleAdd` decision core over injected service results. Telemetry
/// source_type: github sources report the repo, others the source kind.
pub fn decide_add_outcome(
    input: Option<&str>,
    parse_error: Option<&str>,
    add_error: Option<&str>,
    added_name: Option<&str>,
    cli_mode: bool,
) -> AddOutcome {
    let Some(input) = input else {
        return AddOutcome::Error(MSG_EMPTY_INPUT.into());
    };
    if input.trim().is_empty() {
        return AddOutcome::Error(MSG_EMPTY_INPUT.into());
    }
    if let Some(err) = parse_error {
        return AddOutcome::Error(err.to_string());
    }
    if let Some(err) = add_error {
        return AddOutcome::Error(err.to_string());
    }
    let name = added_name.unwrap_or_default().to_string();
    if cli_mode {
        AddOutcome::Success {
            name: name.clone(),
            cli_result: format!("{MSG_SUCCESS_PREFIX}{name}"),
        }
    } else {
        AddOutcome::Success {
            name,
            cli_result: String::new(),
        }
    }
}

/// Telemetry source_type rule: github sources report the repo slug,
/// others the source kind itself.
pub fn telemetry_source_type(parsed: &ParsedSource) -> String {
    match (&parsed.source[..], &parsed.repo) {
        ("github", Some(repo)) => repo.clone(),
        (kind, _) => kind.to_string(),
    }
}

pub fn render_input_view(input_value: &str, error: Option<&str>, is_loading: bool) -> VNode {
    let mut children: Vec<VNode> = vec![
        VNode::Element {
            tag: "TextInput".into(),
            attrs: [
                ("value".into(), json!(input_value)),
                (
                    "placeholder".into(),
                    json!("owner/repo, https://..., or ./path"),
                ),
                ("isLoading".into(), json!(is_loading)),
            ]
            .into_iter()
            .collect(),
            children: vec![],
        },
        VNode::Element {
            tag: "Text".into(),
            attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
            children: vec![VNode::Text(
                "owner/repo, a GitHub URL, a URL, or a local path; Enter to add".into(),
            )],
        },
    ];
    if let Some(err) = error {
        children.push(VNode::Element {
            tag: "Text".into(),
            attrs: [("color".into(), json!("error"))].into_iter().collect(),
            children: vec![VNode::Text(err.into())],
        });
    }
    VNode::Element {
        tag: "Box".into(),
        attrs: [("flexDirection".into(), json!("column"))]
            .into_iter()
            .collect(),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_table_matches_original_messages() {
        assert_eq!(
            decide_add_outcome(Some("  "), None, None, None, false),
            AddOutcome::Error(MSG_EMPTY_INPUT.into())
        );
        assert_eq!(
            decide_add_outcome(None, None, None, None, false),
            AddOutcome::Error(MSG_EMPTY_INPUT.into())
        );
        assert_eq!(
            decide_add_outcome(
                Some("garbage with space"),
                Some(MSG_INVALID_FORMAT.into()),
                None,
                None,
                false
            ),
            AddOutcome::Error(MSG_INVALID_FORMAT.into())
        );
    }

    #[test]
    fn service_errors_flow_to_error_outcome() {
        assert_eq!(
            decide_add_outcome(Some("a/b"), None, Some("network down"), None, false),
            AddOutcome::Error("network down".into())
        );
    }

    #[test]
    fn success_carries_cli_result_only_in_cli_mode() {
        let cli = decide_add_outcome(Some("a/b"), None, None, Some("my-tools"), true);
        assert_eq!(
            cli,
            AddOutcome::Success {
                name: "my-tools".into(),
                cli_result: "Successfully added marketplace: my-tools".into(),
            }
        );
        let ui = decide_add_outcome(Some("a/b"), None, None, Some("my-tools"), false);
        assert_eq!(
            ui,
            AddOutcome::Success {
                name: "my-tools".into(),
                cli_result: String::new()
            }
        );
    }

    #[test]
    fn parse_shapes() {
        assert!(parse_marketplace_input("./local/path").is_ok());
        assert!(parse_marketplace_input("https://github.com/x/y").is_ok());
        assert!(parse_marketplace_input("owner/repo").is_ok());
        assert!(parse_marketplace_input("no slashes at all").is_err());
        assert!(parse_marketplace_input("./").is_err());
    }

    #[test]
    fn telemetry_github_reports_repo() {
        assert_eq!(
            telemetry_source_type(&ParsedSource {
                source: "github".into(),
                repo: Some("o/r".into())
            }),
            "o/r"
        );
        assert_eq!(
            telemetry_source_type(&ParsedSource {
                source: "./path".into(),
                repo: None
            }),
            "./path"
        );
    }

    #[test]
    fn input_view_shows_error_line() {
        match render_input_view("", Some(MSG_EMPTY_INPUT), false) {
            VNode::Element { children, .. } => assert_eq!(children.len(), 3),
            _ => panic!(),
        }
    }
}
