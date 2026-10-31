//! Original slashCommandParsing.ts semantics, preserving literal-space
//! splitting (including repeated spaces) and the "(MCP)" second-word marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSlashCommand {
    pub command_name: String,
    pub args: String,
    pub is_mcp: bool,
}
fn is_ecmascript_trim_char(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'
            | ' '
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

pub fn parse_slash_command(input: &str) -> Option<ParsedSlashCommand> {
    // ECMAScript String.prototype.trim uses the spec's WhiteSpace and
    // LineTerminator set. Unicode White_Space includes U+0085 (NEL), which
    // JS intentionally preserves. Rust char::is_whitespace is too broad.
    let trimmed = input.trim_matches(is_ecmascript_trim_char);
    let without_slash = trimmed.strip_prefix('/')?;
    let words: Vec<&str> = without_slash.split(' ').collect();
    if words.first().copied().unwrap_or("").is_empty() {
        return None;
    }
    let is_mcp = words.get(1).copied() == Some("(MCP)");
    let name = if is_mcp {
        format!("{} (MCP)", words[0])
    } else {
        words[0].to_owned()
    };
    let args = words[if is_mcp { 2 } else { 1 }..].join(" ");
    Some(ParsedSlashCommand {
        command_name: name,
        args,
        is_mcp,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_mcp_and_extra_spaces_match_original() {
        assert_eq!(
            parse_slash_command("/search foo bar"),
            Some(ParsedSlashCommand {
                command_name: "search".into(),
                args: "foo bar".into(),
                is_mcp: false
            })
        );
        assert_eq!(
            parse_slash_command(" /mcp:tool (MCP) arg1 arg2 "),
            Some(ParsedSlashCommand {
                command_name: "mcp:tool (MCP)".into(),
                args: "arg1 arg2".into(),
                is_mcp: true
            })
        );
        assert_eq!(
            parse_slash_command("/foo  bar"),
            Some(ParsedSlashCommand {
                command_name: "foo".into(),
                args: " bar".into(),
                is_mcp: false
            })
        );
        assert_eq!(
            parse_slash_command("/foo  (MCP) bar"),
            Some(ParsedSlashCommand {
                command_name: "foo".into(),
                args: " (MCP) bar".into(),
                is_mcp: false
            })
        );
        assert_eq!(
            parse_slash_command("/foo (MCP)"),
            Some(ParsedSlashCommand {
                command_name: "foo (MCP)".into(),
                args: "".into(),
                is_mcp: true
            })
        );
    }
    #[test]
    fn ecmascript_trim_excludes_nel_and_zero_width_space() {
        // Independently verified against the 2.1.88 TypeScript source via Node.
        assert_eq!(parse_slash_command("\u{0085}/foo bar"), None);
        assert_eq!(parse_slash_command("\u{200B}/foo bar"), None);
        assert_eq!(
            parse_slash_command("/foo bar\u{0085}").unwrap().args,
            "bar\u{0085}"
        );
        for whitespace in ['\u{00A0}', '\u{FEFF}', '\u{2029}', '\u{3000}'] {
            let input = format!("{whitespace}/foo bar{whitespace}");
            let value = parse_slash_command(&input).unwrap();
            assert_eq!(value.command_name, "foo");
            assert_eq!(value.args, "bar");
        }
    }

    #[test]
    fn invalid_inputs_and_literal_space_behavior() {
        for invalid in ["", "abc", "/", "/    ", "   /  foo"] {
            assert_eq!(parse_slash_command(invalid), None, "{invalid}");
        }
        let value = parse_slash_command("\u{feff}/model  x  y\u{feff}").unwrap();
        assert_eq!(value.command_name, "model");
        assert_eq!(value.args, " x  y");
        assert_eq!(
            parse_slash_command("/cmd\tx").unwrap().command_name,
            "cmd\tx"
        );
    }
}
