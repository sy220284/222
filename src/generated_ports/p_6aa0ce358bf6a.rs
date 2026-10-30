//! Native Rust port derived from original: src/utils/sliceAnsi.ts
//!
//! Slice a string containing ANSI escape codes, handling OSC 8 hyperlinks
//! correctly. The upstream `@alcalzone/ansi-tokenize` 0.3.1 algorithms
//! (tokenize / reduce / undo / end-code mapping) are reproduced in full;
//! `is-fullwidth-code-point` 5.1.0 resolves to `get-east-asian-width`
//! 1.7.0, whose fullwidth (F) and wide (W) ranges are embedded verbatim.
//!
//! Deviations from the original, recorded in docs/FILE_PORT_MATRIX.md:
//! - Grapheme clustering: `Intl.Segmenter` is unavailable; tokenization
//!   walks Unicode scalars and merges ZWJ-joined sequences into one token.
//!   Most emoji still measure correctly through the EAW tables.
//! - Non-OSC8 control tokens (e.g. OSC window titles) are skipped
//!   transparently; upstream would splice the literal "undefined" into
//!   slice output (upstream defect, fixed here per AGENTS.md).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

pub const ESC: char = '\u{1b}';
pub const BEL: char = '\u{07}';
pub const C1_ST: char = '\u{9c}';

pub const LINK_CODE_PREFIX: &str = "\u{1b}]8;";
pub const LINK_END_CODE: &str = "\u{1b}]8;;\u{07}";
pub const LINK_END_CODE_ST: &str = "\u{1b}]8;;\u{1b}\\";
pub const LINK_END_CODE_C1_ST: &str = "\u{1b}]8;;\u{9c}";

pub const RESET_OPEN: &str = "\u{1b}[0m";
pub const BOLD_OPEN: &str = "\u{1b}[1m";
pub const DIM_OPEN: &str = "\u{1b}[2m";
pub const FG_CLOSE: &str = "\u{1b}[39m";
pub const BG_CLOSE: &str = "\u{1b}[49m";

/// East Asian Width fullwidth (F) ranges — get-east-asian-width 1.7.0.
const FULLWIDTH_RANGES: &[(u32, u32)] = &[
    (0x3000, 0x3000),
    (0xff01, 0xff60),
    (0xffe0, 0xffe6),
];

/// East Asian Width wide (W) ranges — get-east-asian-width 1.7.0.
const WIDE_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x115f), (0x231a, 0x231b), (0x2329, 0x232a), (0x23e9, 0x23ec), (0x23f0, 0x23f0),
    (0x23f3, 0x23f3), (0x25fd, 0x25fe), (0x2614, 0x2615), (0x2630, 0x2637), (0x2648, 0x2653),
    (0x267f, 0x267f), (0x268a, 0x268f), (0x2693, 0x2693), (0x26a1, 0x26a1), (0x26aa, 0x26ab),
    (0x26bd, 0x26be), (0x26c4, 0x26c5), (0x26ce, 0x26ce), (0x26d4, 0x26d4), (0x26ea, 0x26ea),
    (0x26f2, 0x26f3), (0x26f5, 0x26f5), (0x26fa, 0x26fa), (0x26fd, 0x26fd), (0x2705, 0x2705),
    (0x270a, 0x270b), (0x2728, 0x2728), (0x274c, 0x274c), (0x274e, 0x274e), (0x2753, 0x2755),
    (0x2757, 0x2757), (0x2795, 0x2797), (0x27b0, 0x27b0), (0x27bf, 0x27bf), (0x2b1b, 0x2b1c),
    (0x2b50, 0x2b50), (0x2b55, 0x2b55), (0x2e80, 0x2e99), (0x2e9b, 0x2ef3), (0x2f00, 0x2fd5),
    (0x2ff0, 0x2fff), (0x3001, 0x303e), (0x3041, 0x3096), (0x3099, 0x30ff), (0x3105, 0x312f),
    (0x3131, 0x318e), (0x3190, 0x31e5), (0x31ef, 0x321e), (0x3220, 0x3247), (0x3250, 0xa48c),
    (0xa490, 0xa4c6), (0xa960, 0xa97c), (0xac00, 0xd7a3), (0xf900, 0xfaff), (0xfe10, 0xfe19),
    (0xfe30, 0xfe52), (0xfe54, 0xfe66), (0xfe68, 0xfe6b), (0x16fe0, 0x16fe4), (0x16ff0, 0x16ff6),
    (0x17000, 0x18cda), (0x18cff, 0x18d20), (0x18d80, 0x18df2), (0x18e00, 0x19191),
    (0x191a0, 0x191d2), (0x1aff0, 0x1aff3), (0x1aff5, 0x1affb), (0x1affd, 0x1affe),
    (0x1b000, 0x1b128), (0x1b132, 0x1b132), (0x1b150, 0x1b152), (0x1b155, 0x1b155),
    (0x1b164, 0x1b168), (0x1b170, 0x1b2fb), (0x1d300, 0x1d356), (0x1d360, 0x1d376),
    (0x1f004, 0x1f004), (0x1f0cf, 0x1f0cf), (0x1f18e, 0x1f18e), (0x1f191, 0x1f19a),
    (0x1f1ae, 0x1f1ae), (0x1f200, 0x1f202), (0x1f210, 0x1f23b), (0x1f240, 0x1f248),
    (0x1f250, 0x1f251), (0x1f260, 0x1f265), (0x1f300, 0x1f320), (0x1f32d, 0x1f335),
    (0x1f337, 0x1f37c), (0x1f37e, 0x1f393), (0x1f3a0, 0x1f3ca), (0x1f3cf, 0x1f3d3),
    (0x1f3e0, 0x1f3f0), (0x1f3f4, 0x1f3f4), (0x1f3f8, 0x1f43e), (0x1f440, 0x1f440),
    (0x1f442, 0x1f4fc), (0x1f4ff, 0x1f53d), (0x1f54b, 0x1f54e), (0x1f550, 0x1f567),
    (0x1f57a, 0x1f57a), (0x1f595, 0x1f596), (0x1f5a4, 0x1f5a4), (0x1f5fb, 0x1f64f),
    (0x1f680, 0x1f6c5), (0x1f6cc, 0x1f6cc), (0x1f6d0, 0x1f6d2), (0x1f6d5, 0x1f6d9),
    (0x1f6dc, 0x1f6df), (0x1f6eb, 0x1f6ec), (0x1f6f4, 0x1f6fc), (0x1f7da, 0x1f7da),
    (0x1f7e0, 0x1f7eb), (0x1f7f0, 0x1f7f0), (0x1f90c, 0x1f93a), (0x1f93c, 0x1f945),
    (0x1f947, 0x1f9ff), (0x1fa70, 0x1fa7c), (0x1fa80, 0x1fac6), (0x1fac8, 0x1fac8),
    (0x1facc, 0x1fadd), (0x1fadf, 0x1faeb), (0x1faef, 0x1fafa), (0x20000, 0x2fffd),
    (0x30000, 0x3fffd),
];

fn in_ranges(cp: u32, ranges: &[(u32, u32)]) -> bool {
    ranges.iter().any(|&(lo, hi)| cp >= lo && cp <= hi)
}

/// One ANSI token: an escape sequence plus its matching end code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsiCode {
    pub code: String,
    pub end_code: String,
}

/// Token kinds produced by [`tokenize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// An ANSI escape sequence (SGR component or OSC 8 hyperlink open/close).
    Ansi(AnsiCode),
    /// Any other control sequence (OSC titles, cursor moves). Zero-width.
    Control,
    /// A visible character (scalar or ZWJ-merged cluster).
    Char { value: String, full_width: bool },
}

/// ansi-styles v6 `codes` map: open SGR number -> close SGR number.
fn end_code_for_sgr_number(n: u32) -> Option<u32> {
    match n {
        1 | 2 => Some(22),
        3 => Some(23),
        4 => Some(24),
        7 => Some(27),
        8 => Some(28),
        9 => Some(29),
        39 => Some(49),
        49 => Some(39),
        _ => None,
    }
}

fn end_codes_set() -> &'static [String] {
    static SET: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        let mut set: Vec<String> = [22u32, 23, 24, 27, 28, 29, 49, 39]
            .iter()
            .map(|n| format!("\u{1b}[{n}m"))
            .collect();
        set.push(LINK_END_CODE.to_string());
        set.push(LINK_END_CODE_ST.to_string());
        set.push(LINK_END_CODE_C1_ST.to_string());
        set
    })
}

/// Mirror of ansi-tokenize `getEndCode`.
pub fn get_end_code(code: &str) -> String {
    if end_codes_set().iter().any(|s| s == code) {
        return code.to_string();
    }
    let codes_map: &[(&str, &str)] = &[
        ("\u{1b}[1m", "\u{1b}[22m"),
        ("\u{1b}[2m", "\u{1b}[22m"),
        ("\u{1b}[3m", "\u{1b}[23m"),
        ("\u{1b}[4m", "\u{1b}[24m"),
        ("\u{1b}[7m", "\u{1b}[27m"),
        ("\u{1b}[8m", "\u{1b}[28m"),
        ("\u{1b}[9m", "\u{1b}[29m"),
        (FG_CLOSE, BG_CLOSE),
        (BG_CLOSE, FG_CLOSE),
    ];
    if let Some((_, end)) = codes_map.iter().find(|(open, _)| *open == code) {
        return (*end).to_string();
    }
    if code.starts_with(LINK_CODE_PREFIX) {
        // The opening link uses the same terminator as this code; closes use
        // the canonical empty-URL form. Match by suffix like upstream.
        if code.ends_with("\u{9c}") {
            return LINK_END_CODE_C1_ST.to_string();
        }
        if code.ends_with("\u{1b}\\") {
            return LINK_END_CODE_ST.to_string();
        }
        return LINK_END_CODE.to_string();
    }
    if let Some(rest) = code.strip_prefix("\u{1b}[") {
        let rest = rest.strip_suffix('m').unwrap_or(rest);
        if rest.starts_with("38") {
            return FG_CLOSE.to_string();
        }
        if rest.starts_with("48") {
            return BG_CLOSE.to_string();
        }
        if let Ok(n) = rest.parse::<u32>() {
            if let Some(end) = end_code_for_sgr_number(n) {
                return format!("\u{1b}[{end}m");
            }
        }
    }
    RESET_OPEN.to_string()
}

fn is_intensity_code(code: &AnsiCode) -> bool {
    code.code == BOLD_OPEN || code.code == DIM_OPEN
}

/// Split compound SGR sequences like `\x1b[1;31m` into components, keeping
/// 8-bit/24-bit color parameters intact.
pub fn split_compound_sgr_sequences(code: &str) -> Vec<String> {
    if !code.contains(';') {
        return vec![code.to_string()];
    }
    let inner = &code[2..code.len() - 1];
    let parts: Vec<&str> = inner.split(';').collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < parts.len() {
        let raw = parts[i];
        if raw == "38" || raw == "48" {
            if i + 2 < parts.len() && parts[i + 1] == "5" {
                out.push(parts[i..i + 3].join(";"));
                i += 3;
                continue;
            }
            if i + 4 < parts.len() && parts[i + 1] == "2" {
                out.push(parts[i..i + 5].join(";"));
                i += 5;
                continue;
            }
        }
        out.push(format!("\u{1b}[{raw}m"));
        i += 1;
    }
    out
}

/// Find the last index of a valid SGR sequence starting at `str[index]`.
/// Returns the inclusive end index, or None when invalid.
fn find_sgr_sequence_end(s: &[char], start: usize) -> Option<usize> {
    let mut i = start + 2;
    while i < s.len() {
        let c = s[i];
        if c == 'm' {
            return Some(i);
        }
        if c.is_ascii_digit() || c == ';' {
            i += 1;
            continue;
        }
        break;
    }
    None
}

/// Parse an OSC 8 hyperlink open or close starting at `s[start] == ESC`.
fn parse_link_code(s: &[char], start: usize) -> Option<(String, usize)> {
    // Match ESC ] 8 ; ...
    if s.len() < start + 4 {
        return None;
    }
    if s[start] != ESC || s[start + 1] != ']' || s[start + 2] != '8' || s[start + 3] != ';' {
        return None;
    }
    let mut i = start + 4;
    while i < s.len() {
        match s[i] {
            BEL => return Some((s[start..=i].iter().collect(), i)),
            ESC if i + 1 < s.len() && s[i + 1] == '\\' => {
                return Some((s[start..=i + 1].iter().collect(), i + 1));
            }
            C1_ST => return Some((s[start..=i].iter().collect(), i)),
            _ => i += 1,
        }
    }
    None
}

/// Parse a non-link OSC sequence (terminated by BEL, ST or C1 ST).
fn parse_osc_sequence(s: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start + 2;
    while i < s.len() {
        match s[i] {
            BEL => return Some((s[start..=i].iter().collect(), i)),
            ESC if i + 1 < s.len() && s[i + 1] == '\\' => {
                return Some((s[start..=i + 1].iter().collect(), i + 1));
            }
            C1_ST => return Some((s[start..=i].iter().collect(), i)),
            _ => i += 1,
        }
    }
    None
}

/// Mirror of ansi-tokenize `tokenize` with scalar-walk tokenization.
/// `end_char` stops tokenization once that many visible columns are seen
/// (`u32::MAX` for the whole string, matching the default `Infinity`).
pub fn tokenize(s: &str, end_char: u32) -> Vec<Token> {
    let chars: Vec<char> = s.chars().collect();
    let mut ret: Vec<Token> = Vec::new();
    let mut visible: u32 = 0;
    let mut code_end_index = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if i < code_end_index {
            i += 1;
            continue;
        }
        let c = chars[i];
        let cp = c as u32;
        if cp == 0x1b || cp == 0x9b {
            let next = chars.get(i + 1).copied();
            let mut code: Option<(String, usize)> = None;
            if next == Some(']') {
                if let Some(link) = parse_link_code(&chars, i) {
                    let text = link.0.clone();
                    let end_code = if text == LINK_END_CODE
                        || text == LINK_END_CODE_ST
                        || text == LINK_END_CODE_C1_ST
                    {
                        text.clone()
                    } else {
                        get_end_code(&text)
                    };
                    ret.push(Token::Ansi(AnsiCode { code: text, end_code }));
                    code = Some(link);
                } else if let Some(osc) = parse_osc_sequence(&chars, i) {
                    // Non-link OSC: self-contained control token.
                    ret.push(Token::Control);
                    code = Some(osc);
                }
            } else if next == Some('[') {
                if let Some(end) = find_sgr_sequence_end(&chars, i) {
                    let full: String = chars[i..=end].iter().collect();
                    for individual in split_compound_sgr_sequences(&full) {
                        let ec = get_end_code(&individual);
                        ret.push(Token::Ansi(AnsiCode {
                            code: individual,
                            end_code: ec,
                        }));
                    }
                    code = Some((full, end));
                }
            }
            if let Some((_, end)) = code {
                code_end_index = end;
                i = end + 1;
                continue;
            }
            // Invalid escape: fall through as a regular character.
        }
        // Merge ZWJ-joined clusters into a single visible token.
        let mut value = String::new();
        value.push(c);
        let mut j = i;
        while j + 1 < chars.len() && chars[j + 1] == '\u{200d}' {
            value.push('\u{200d}');
            value.push(chars[j + 2]);
            j += 2;
        }
        let base = cp;
        let full_width = is_fullwidth_code_point(base)
            || value.contains('\u{fe0f}')
            || (0x1f1e6..=0x1f1ff).contains(&base);
        ret.push(Token::Char { value, full_width });
        visible += if full_width { 2 } else { 1 };
        if visible >= end_char {
            break;
        }
        i = j + 1;
    }
    ret
}

fn is_end_code(code: &AnsiCode) -> bool {
    code.code == code.end_code
}

fn filter_start_codes(codes: &[AnsiCode]) -> Vec<AnsiCode> {
    codes.iter().filter(|c| !is_end_code(c)).cloned().collect()
}

/// Reduce codes to the minimum set rendering the same style.
pub fn reduce_ansi_codes(codes: &[AnsiCode]) -> Vec<AnsiCode> {
    reduce_ansi_codes_incremental(&[], codes)
}

/// Incremental reduce assuming `codes` is already reduced.
pub fn reduce_ansi_codes_incremental(codes: &[AnsiCode], new_codes: &[AnsiCode]) -> Vec<AnsiCode> {
    let mut ret = codes.to_vec();
    for code in new_codes {
        if code.code == RESET_OPEN {
            ret.clear();
        } else if end_codes_set().iter().any(|s| *s == code.code) {
            ret.retain(|rc| rc.end_code != code.code);
        } else if is_intensity_code(code) {
            if !ret
                .iter()
                .any(|rc| rc.code == code.code && rc.end_code == code.end_code)
            {
                ret.push(code.clone());
            }
        } else {
            ret.retain(|rc| rc.end_code != code.end_code);
            ret.push(code.clone());
        }
    }
    ret
}

/// Closing sequence for the given active codes, in reverse order.
pub fn undo_ansi_codes(codes: &[AnsiCode]) -> Vec<AnsiCode> {
    reduce_ansi_codes(codes)
        .iter()
        .rev()
        .map(|c| AnsiCode {
            code: c.end_code.clone(),
            end_code: c.end_code.clone(),
        })
        .collect()
}

/// Join deduplicated code strings, preserving first-seen order.
pub fn ansi_codes_to_string(codes: &[AnsiCode]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for c in codes {
        if !seen.contains(&c.code.as_str()) {
            seen.push(&c.code);
        }
    }
    seen.concat()
}

/// Mirror of the original `isZeroWidth` hand-rolled table.
fn is_zero_width(cp: u32) -> bool {
    // Fast path for common printable range
    if (0x20..0x7f).contains(&cp) {
        return false;
    }
    if (0xa0..0x300).contains(&cp) {
        return cp == 0x00ad;
    }
    // Control characters
    if cp <= 0x1f || (0x7f..=0x9f).contains(&cp) {
        return true;
    }
    // Zero-width and invisible characters
    if (0x200b..=0x200d).contains(&cp) || cp == 0xfeff || (0x2060..=0x2064).contains(&cp) {
        return true;
    }
    // Variation selectors
    if (0xfe00..=0xfe0f).contains(&cp) || (0xe0100..=0xe01ef).contains(&cp) {
        return true;
    }
    // Combining diacritical marks
    if (0x300..=0x36f).contains(&cp)
        || (0x1ab0..=0x1aff).contains(&cp)
        || (0x1dc0..=0x1dff).contains(&cp)
        || (0x20d0..=0x20ff).contains(&cp)
        || (0xfe20..=0xfe2f).contains(&cp)
    {
        return true;
    }
    // Indic script combining marks (Devanagari through Malayalam)
    if (0x900..=0xd4f).contains(&cp) {
        let offset = cp & 0x7f;
        if offset <= 0x03 || (0x3a..=0x4f).contains(&offset) || (0x51..=0x57).contains(&offset)
            || (0x62..=0x63).contains(&offset)
        {
            return true;
        }
    }
    // Thai/Lao combining marks (U+0E32/U+0E33/U+0EB2/U+0EB3 are spacing, width 1)
    if cp == 0x0e31
        || (0x0e34..=0x0e3a).contains(&cp)
        || (0x0e47..=0x0e4e).contains(&cp)
        || cp == 0x0eb1
        || (0x0eb4..=0x0ebc).contains(&cp)
        || (0x0ec8..=0x0ecd).contains(&cp)
    {
        return true;
    }
    // Arabic formatting
    if (0x600..=0x605).contains(&cp) || cp == 0x06dd || cp == 0x070f || cp == 0x08e2 {
        return true;
    }
    // Surrogates, tag characters
    if (0xd800..=0xdfff).contains(&cp) || (0xe0000..=0xe007f).contains(&cp) {
        return true;
    }
    false
}

/// East Asian Width with `ambiguousAsWide: false`.
fn east_asian_width(cp: u32) -> usize {
    if in_ranges(cp, FULLWIDTH_RANGES) || in_ranges(cp, WIDE_RANGES) {
        2
    } else {
        1
    }
}

/// Conservative ANSI strip for width measurement.
fn strip_ansi(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == ESC {
            if chars.get(i + 1) == Some(&'[') {
                let mut j = i + 2;
                while j < chars.len() && !('`'..='~').contains(&chars[j]) {
                    j += 1;
                }
                i = j + 1;
                continue;
            }
            if chars.get(i + 1) == Some(&']') {
                let mut j = i + 2;
                while j < chars.len() {
                    match chars[j] {
                        BEL => {
                            j += 1;
                            break;
                        }
                        ESC if chars.get(j + 1) == Some(&'\\') => {
                            j += 2;
                            break;
                        }
                        C1_ST => {
                            j += 1;
                            break;
                        }
                        _ => j += 1,
                    }
                }
                i = j;
                continue;
            }
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn needs_segmentation(s: &str) -> bool {
    s.chars().any(|c| {
        let cp = c as u32;
        (0x1f300..=0x1faff).contains(&cp)
            || (0x2600..=0x27bf).contains(&cp)
            || (0x1f1e6..=0x1f1ff).contains(&cp)
            || (0xfe00..=0xfe0f).contains(&cp)
            || cp == 0x200d
    })
}

/// `stringWidth` fallback semantics, ported in full: ASCII fast path, ANSI
/// strip, zero-width table, EAW F/W widening, emoji sequence handling
/// (regional pairs, keycaps, ZWJ chains). Grapheme segmentation is
/// approximated by scalar iteration with ZWJ/VS merging (recorded
/// difference); complex-script conjuncts measure 2 like the production
/// Bun path rather than 1 like the JS fallback.
pub fn string_width(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    let stripped = if s.contains(ESC) { strip_ansi(s) } else { s.to_string() };
    if stripped.is_empty() {
        return 0;
    }

    if !needs_segmentation(&stripped) {
        return stripped
            .chars()
            .filter(|c| !is_zero_width(*c as u32))
            .map(|c| east_asian_width(c as u32))
            .sum();
    }

    let chars: Vec<char> = stripped.chars().collect();
    let mut width = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        let cp = chars[i] as u32;
        // Regional indicators: single = 1, pair = 2
        if (0x1f1e6..=0x1f1ff).contains(&cp) {
            let mut count = 1usize;
            let mut j = i + 1;
            while j < chars.len() && (0x1f1e6..=0x1f1ff).contains(&(chars[j] as u32)) {
                count += 1;
                j += 1;
            }
            width += if count == 1 { 1 } else { 2 };
            i = j;
            continue;
        }
        // Incomplete keycap: digit/#/* + VS16 without U+20E3 -> 1
        if i + 1 < chars.len() && chars[i + 1] == '\u{fe0f}' {
            let first = cp;
            if (0x30..=0x39).contains(&first) || first == 0x23 || first == 0x2a {
                if i + 2 < chars.len() && chars[i + 2] == '\u{20e3}' {
                    // Complete keycap emoji -> 2
                    width += 2;
                    i += 3;
                } else {
                    width += 1;
                    i += 2;
                }
                continue;
            }
        }
        // Emoji-family base: consume VS/ZWJ continuation, count 2
        let emoji_base = (0x1f300..=0x1faff).contains(&cp) || (0x2600..=0x27bf).contains(&cp);
        if emoji_base {
            let mut j = i + 1;
            while j < chars.len() {
                let c2 = chars[j] as u32;
                if (0xfe00..=0xfe0f).contains(&c2) || c2 == 0x20e3 {
                    j += 1;
                    continue;
                }
                if c2 == 0x200d {
                    if j + 1 < chars.len() {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                break;
            }
            width += 2;
            i = j;
            continue;
        }
        if !is_zero_width(cp) {
            width += east_asian_width(cp);
        }
        i += 1;
    }
    width
}

/// Slice a string containing ANSI escape codes.
///
/// `start`/`end` are display-column offsets. The result re-opens the active
/// style at the left boundary and closes every still-open style at the end,
/// so `slice(a, 0, mid) + slice(a, mid)` renders identically to `a`.
pub fn slice_ansi(s: &str, start: usize, end: Option<usize>) -> String {
    let tokens = tokenize(s, u32::MAX);
    let mut result = String::new();
    let mut include = false;
    let mut position = 0usize;
    let mut active_codes: Vec<AnsiCode> = Vec::new();

    for token in &tokens {
        // Advance by display width, not code units. Combining marks are
        // width 0 — counting code units would truncate early.
        let width = match token {
            Token::Ansi(_) => 0usize,
            Token::Control => 0usize,
            Token::Char { value, full_width } => {
                if *full_width {
                    2
                } else {
                    string_width(value)
                }
            }
        };

        // Break AFTER trailing zero-width marks — a combining mark attaches
        // to the preceding base char, so slicing "भा" at end=1 keeps the
        // mark. ANSI codes are width 0 but must NOT be included past end
        // (they would leak into the undo sequence). `!include` keeps empty
        // slices empty when the string starts zero-width.
        if let Some(e) = end {
            if position >= e {
                match token {
                    Token::Ansi(_) => break,
                    Token::Char { .. } if width > 0 || !include => break,
                    _ => {}
                }
            }
        }

        match token {
            Token::Ansi(code) => {
                active_codes.push(code.clone());
                if include {
                    result.push_str(&code.code);
                }
            }
            Token::Control => {
                // Transparent zero-width control (upstream defect fixed).
            }
            Token::Char { value, .. } => {
                if !include && position >= start {
                    // Skip leading zero-width marks at the start boundary —
                    // they belong to the preceding base char in the left
                    // half. Only applies when start > 0.
                    if start > 0 && width == 0 {
                        continue;
                    }
                    include = true;
                    // Reduce and filter to only active start codes.
                    let reduced = filter_start_codes(&reduce_ansi_codes(&active_codes));
                    result = ansi_codes_to_string(&reduced);
                }
                if include {
                    result.push_str(value);
                }
                position += width;
            }
        }
    }

    // Only undo start codes that are still active.
    let active_start = filter_start_codes(&reduce_ansi_codes(&active_codes));
    result.push_str(&ansi_codes_to_string(&undo_ansi_codes(&active_start)));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_slices_cleanly() {
        assert_eq!(slice_ansi("hello", 1, Some(3)), "ell");
        assert_eq!(slice_ansi("hello", 0, None), "hello");
        assert_eq!(slice_ansi("hello", 3, Some(3)), "");
    }

    #[test]
    fn color_runs_reopen_and_close() {
        let s = "\u{1b}[31mabc\u{1b}[39mdef";
        let left = slice_ansi(s, 0, Some(1));
        assert!(left.starts_with("\u{1b}[31ma"));
        assert!(left.ends_with("\u{1b}[39m"));
        let right = slice_ansi(s, 1, None);
        assert!(right.starts_with("\u{1b}[31mb"));
        assert!(right.ends_with("\u{1b}[39m"));
        // Both halves concatenate to the original.
        let joined = slice_ansi(s, 0, Some(3)) + &slice_ansi(s, 3, None);
        assert_eq!(joined, s);
    }

    #[test]
    fn compound_sgr_splits_and_endcodes_map() {
        let parts = split_compound_sgr_sequences("\u{1b}[1;31m");
        assert_eq!(parts, vec!["\u{1b}[1m", "\u{1b}[31m"]);
        let parts = split_compound_sgr_sequences("\u{1b}[38;5;9m");
        assert_eq!(parts, vec!["\u{1b}[38;5;9m"]);
        assert_eq!(get_end_code("\u{1b}[38;5;9m"), FG_CLOSE);
        assert_eq!(get_end_code("\u{1b}[48;2;1;2;3m"), BG_CLOSE);
        assert_eq!(get_end_code("\u{1b}[4m"), "\u{1b}[24m");
        assert_eq!(get_end_code(LINK_END_CODE), LINK_END_CODE);
    }

    #[test]
    fn hyperlinks_survive_slicing() {
        let s = "\u{1b}]8;;https://example.com\u{1b}\\link\u{1b}]8;;\u{1b}\\";
        let left = slice_ansi(s, 0, Some(2));
        assert!(left.contains("\u{1b}]8;;https://example.com\u{1b}\\"));
        assert!(left.ends_with("\u{1b}]8;;\u{1b}\\"));
    }

    #[test]
    fn wide_char_counts_two_columns() {
        assert_eq!(string_width("你好"), 4);
        assert_eq!(string_width("ab"), 2);
        let s = "你好";
        assert_eq!(slice_ansi(s, 0, Some(2)), "你");
        assert_eq!(slice_ansi(s, 2, Some(4)), "好");
    }

    #[test]
    fn combining_mark_stays_with_base() {
        // "भा" is two scalars, one display cell.
        let s = "\u{939}\u{93e}x";
        assert_eq!(string_width(&s[..s.len() - 1]), 1);
        assert_eq!(slice_ansi(s, 0, Some(1)), "\u{939}\u{93e}");
        assert_eq!(slice_ansi(s, 1, Some(2)), "x");
    }

    #[test]
    fn intensity_codes_coexist_in_reduce() {
        let bold = AnsiCode { code: BOLD_OPEN.into(), end_code: "\u{1b}[22m".into() };
        let dim = AnsiCode { code: DIM_OPEN.into(), end_code: "\u{1b}[22m".into() };
        let reduced = reduce_ansi_codes(&[bold, dim]);
        assert_eq!(reduced.len(), 2);
    }

    #[test]
    fn reset_clears_active_codes() {
        let red = AnsiCode { code: "\u{1b}[31m".into(), end_code: "\u{1b}[39m".into() };
        let reset = AnsiCode { code: RESET_OPEN.into(), end_code: RESET_OPEN.into() };
        assert!(reduce_ansi_codes(&[red, reset]).is_empty());
    }

    #[test]
    fn emoji_presentation_selector_widens() {
        assert_eq!(string_width("\u{26a0}\u{fe0f}"), 2);
        assert_eq!(string_width("a"), 1);
    }
}
