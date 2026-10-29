//! Terminal control, PDF range and document capability helpers ported from
//! 2.1.88 src/ink/clearTerminal.ts and src/utils/pdfUtils.ts.
use serde_json::{json, Value};

/// Native terminal capability detection preserving Windows Terminal, Mintty,
/// VS Code ConPTY and legacy console clear/scrollback differences.
pub fn get_clear_terminal_sequence_for(
    windows: bool,
    wt_session: bool,
    term_program: Option<&str>,
    term_program_version: Option<&str>,
    msystem: bool,
) -> String {
    let mintty = term_program == Some("mintty") || (windows && msystem);
    let modern = wt_session
        || windows
            && term_program == Some("vscode")
            && term_program_version.is_some_and(|version| !version.is_empty())
        || mintty;
    if windows && !modern {
        "\u{1b}[2J\u{1b}[0f".into()
    } else {
        "\u{1b}[2J\u{1b}[3J\u{1b}[H".into()
    }
}

pub fn get_clear_terminal_sequence() -> String {
    get_clear_terminal_sequence_for(
        cfg!(windows),
        std::env::var_os("WT_SESSION").is_some(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("TERM_PROGRAM_VERSION").ok().as_deref(),
        std::env::var_os("MSYSTEM").is_some(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRange {
    pub first_page: u64,
    /// None represents an open-ended range, equivalent to JS Infinity.
    pub last_page: Option<u64>,
}

/// JavaScript parseInt(x,10) scans a decimal prefix, including leading
/// plus signs and whitespace. Preserve that original behavior.
fn javascript_parse_int(input: &str) -> Option<u64> {
    let trimmed = input.trim_start();
    let unsigned = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if unsigned.starts_with('-') {
        return None;
    }
    let digits: String = unsigned.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub fn parse_pdf_page_range(pages: &str) -> Option<PageRange> {
    let s = pages.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(open) = s.strip_suffix('-') {
        let first = javascript_parse_int(open)?;
        return (first > 0).then_some(PageRange {
            first_page: first,
            last_page: None,
        });
    }
    if let Some((first, last)) = s.split_once('-') {
        let first = javascript_parse_int(first)?;
        let last = javascript_parse_int(last)?;
        return (first > 0 && last >= first).then_some(PageRange {
            first_page: first,
            last_page: Some(last),
        });
    }
    let page = javascript_parse_int(s)?;
    (page > 0).then_some(PageRange {
        first_page: page,
        last_page: Some(page),
    })
}

pub fn is_pdf_extension(ext: &str) -> bool {
    ext.strip_prefix('.')
        .unwrap_or(ext)
        .eq_ignore_ascii_case("pdf")
}

pub fn is_pdf_supported(model: &str) -> bool {
    // The original excludes only the legacy Haiku 3 identifier.
    !model.to_ascii_lowercase().contains("claude-3-haiku")
}

pub fn exports(source: &str) -> Option<&'static [&'static str]> {
    match source {
        "src/utils/pdfUtils.ts" => Some(&[
            "parsePDFPageRange",
            "isPDFSupported",
            "isPDFExtension",
        ]),
        "src/ink/clearTerminal.ts" => Some(&[
            "getClearTerminalSequence",
            "clearTerminal",
        ]),
        _ => None,
    }
}

pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    let arg = args.first().and_then(Value::as_str).unwrap_or("");
    match (source, export) {
        ("src/utils/pdfUtils.ts", "parsePDFPageRange") => {
            Some(parse_pdf_page_range(arg).map_or(Value::Null, |p| {
                json!({"firstPage":p.first_page,"lastPage":p.last_page})
            }))
        }
        ("src/utils/pdfUtils.ts", "isPDFSupported") => Some(json!(is_pdf_supported(arg))),
        ("src/utils/pdfUtils.ts", "isPDFExtension") => Some(json!(is_pdf_extension(arg))),
        ("src/ink/clearTerminal.ts", "getClearTerminalSequence" | "clearTerminal") => {
            Some(json!(get_clear_terminal_sequence()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges_follow_source_parseint_semantics() {
        assert_eq!(
            parse_pdf_page_range("5"),
            Some(PageRange { first_page: 5, last_page: Some(5) })
        );
        assert_eq!(
            parse_pdf_page_range("1-10"),
            Some(PageRange { first_page: 1, last_page: Some(10) })
        );
        assert_eq!(
            parse_pdf_page_range("3-"),
            Some(PageRange { first_page: 3, last_page: None })
        );
        assert_eq!(
            parse_pdf_page_range("1abc-2junk"),
            Some(PageRange { first_page: 1, last_page: Some(2) })
        );
        for input in ["", "-3", "0", "1-0", "10-2", "abc", "5-abc"] {
            assert_eq!(parse_pdf_page_range(input), None);
        }
        assert!(is_pdf_extension(".PDF"));
        assert!(!is_pdf_extension("pdfx"));
        assert!(!is_pdf_supported("bedrock.anthropic.claude-3-haiku-20240307"));
        assert!(is_pdf_supported("gpt-6"));
    }

    #[test]
    fn clear_terminal_capability_uses_source_windows_matrix() {
        assert_eq!(
            get_clear_terminal_sequence_for(false, false, None, None, false),
            "\u{1b}[2J\u{1b}[3J\u{1b}[H"
        );
        assert_eq!(
            get_clear_terminal_sequence_for(true, false, None, None, false),
            "\u{1b}[2J\u{1b}[0f"
        );
        for (terminal, version, msystem, wt) in [
            (None, None, false, true),
            (Some("mintty"), None, false, false),
            (None, None, true, false),
            (Some("vscode"), Some("1.0"), false, false),
        ] {
            assert_eq!(
                get_clear_terminal_sequence_for(true, wt, terminal, version, msystem),
                "\u{1b}[2J\u{1b}[3J\u{1b}[H"
            );
        }
    }
}
