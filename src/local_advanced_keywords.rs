//! Source-aligned local keyword trigger detection from
//! 2.1.88 src/utils/ultraplan/keyword.ts. No vendor cloud dependency.
use std::ops::Range;

fn word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn preceding(text: &str, byte: usize) -> Option<char> {
    text.get(..byte)?.chars().next_back()
}

fn following(text: &str, byte: usize) -> Option<char> {
    text.get(byte..)?.chars().next()
}

fn matching_ranges(text: &str, keyword: &str) -> Vec<Range<usize>> {
    if text.starts_with('/') {
        return Vec::new();
    }
    // JS RegExp \\b in the source uses ASCII-word boundaries. The keyword
    // itself contains ASCII letters only; surrounding Unicode letters do not
    // suppress its ASCII boundary.
    let mut ranges = Vec::new();
    let lower = text.to_ascii_lowercase();
    for (start, _) in lower.match_indices(keyword) {
        let end = start + keyword.len();
        let before = preceding(text, start);
        let after = following(text, end);
        let ascii_word = |c: Option<char>| {
            c.is_some_and(|x| x.is_ascii_alphanumeric() || x == '_')
        };
        if ascii_word(before) || ascii_word(after) {
            continue;
        }
        if matches!(before, Some('/' | '\\' | '-'))
            || matches!(after, Some('/' | '\\' | '-' | '?'))
            || (after == Some('.') && word_char(following(text, end + 1)))
        {
            continue;
        }
        ranges.push(start..end);
    }
    if ranges.is_empty() {
        return ranges;
    }
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut quoted = Vec::<Range<usize>>::new();
    let mut opened: Option<(char, usize)> = None;
    for (i, &(start, c)) in chars.iter().enumerate() {
        if let Some((quote, begin)) = opened {
            if quote == '[' && c == '[' {
                opened = Some((quote, start));
                continue;
            }
            let closing = match quote {
                '<' => '>',
                '{' => '}',
                '[' => ']',
                '(' => ')',
                other => other,
            };
            if c != closing {
                continue;
            }
            if quote == '\'' && word_char(chars.get(i + 1).map(|(_, ch)| *ch)) {
                continue;
            }
            quoted.push(begin..(start + c.len_utf8()));
            opened = None;
        } else {
            let before = if i > 0 { Some(chars[i - 1].1) } else { None };
            let after = chars.get(i + 1).map(|(_, c)| *c);
            let is_angle = c == '<' && after.is_some_and(|next| next.is_ascii_alphabetic() || next == '/');
            let is_single = c == '\'' && !word_char(before);
            let is_pair = matches!(c, '`' | '"' | '{' | '[' | '(');
            if is_angle || is_single || is_pair {
                opened = Some((c, start));
            }
        }
    }
    ranges.retain(|r| !quoted.iter().any(|q| q.contains(&r.start)));
    ranges
}

pub fn ultraplan_positions(text: &str) -> Vec<Range<usize>> {
    matching_ranges(text, "ultraplan")
}
pub fn ultrareview_positions(text: &str) -> Vec<Range<usize>> {
    matching_ranges(text, "ultrareview")
}

/// Replace only the first valid trigger, preserving its suffix casing. An
/// isolated keyword is a usage request: there is no task to execute.
pub fn replace_ultraplan_keyword(text: &str) -> Option<String> {
    let first = ultraplan_positions(text).into_iter().next()?;
    let before = &text[..first.start];
    let after = &text[first.end..];
    if format!("{before}{after}").trim().is_empty() {
        return Some(String::new());
    }
    Some(format!("{before}{}{after}", &text[first.start + "ultra".len()..first.end]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_source_path_quote_and_question_rules() {
        for text in [
            "/rename ultraplan task",
            "`ultraplan`",
            "\"ultraplan\"",
            "'ultraplan'",
            "<ultraplan>",
            "(ultraplan)",
            "[Pasted ultraplan text]",
            "src/ultraplan/foo.ts",
            "ultraplan.tsx",
            "--ultraplan-mode",
            "ultraplan?",
            "ultraplan-s",
        ] {
            assert!(ultraplan_positions(text).is_empty(), "{text}");
        }
        assert!(ultraplan_positions("please ultraplan this.").len() == 1);
        assert_eq!(ultraplan_positions("let's ultraplan it's").len(), 1);
        assert_eq!(ultrareview_positions("please ultrareview this!").len(), 1);
        assert_eq!(ultrareview_positions("/review ultrareview").len(), 0);
    }
    #[test]
    fn replacement_preserves_case_and_avoids_empty_prompt() {
        assert_eq!(replace_ultraplan_keyword("please ultraplan this").as_deref(), Some("please plan this"));
        assert_eq!(replace_ultraplan_keyword("please ULTRAPLAN this").as_deref(), Some("please PLAN this"));
        assert_eq!(replace_ultraplan_keyword("ultraplan").as_deref(), Some(""));
        assert!(replace_ultraplan_keyword("src/ultraplan.tsx").is_none());
    }
    #[test]
    fn paired_delimiters_do_not_hide_later_trigger() {
        let text = "`ultraplan` please ultraplan this";
        assert_eq!(ultraplan_positions(text).len(), 1);
        assert_eq!(replace_ultraplan_keyword(text).as_deref(), Some("`ultraplan` please plan this"));
    }
}
