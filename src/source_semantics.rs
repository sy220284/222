//! Source-level Rust implementation for 2.1.88 terminal escape generation,
//! ANSI-to-SVG rendering and SDK streamlined output transformation.
//! No original JavaScript is executed at runtime.
#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// src/ink/termio/{ansi,csi,dec}.ts
pub fn is_c0(byte: i64) -> bool {
    byte < 0x20 || byte == 0x7f
}
pub fn is_esc_final(byte: i64) -> bool {
    (0x30..=0x7e).contains(&byte)
}
pub fn is_csi_param(byte: i64) -> bool {
    (0x30..=0x3f).contains(&byte)
}
pub fn is_csi_intermediate(byte: i64) -> bool {
    (0x20..=0x2f).contains(&byte)
}
pub fn is_csi_final(byte: i64) -> bool {
    (0x40..=0x7e).contains(&byte)
}
pub fn csi(args: &[Value]) -> String {
    let body = if args.len() <= 1 {
        args.first().map(js_string).unwrap_or_default()
    } else {
        let mut parts = args[..args.len() - 1]
            .iter()
            .map(js_string)
            .collect::<Vec<_>>()
            .join(";");
        parts.push_str(&js_string(args.last().unwrap()));
        parts
    };
    format!("\x1b[{body}")
}
pub fn decset(mode: i64) -> String {
    format!("\x1b[?{mode}h")
}
pub fn decreset(mode: i64) -> String {
    format!("\x1b[?{mode}l")
}
pub fn cursor_motion(direction: char, n: i64) -> String {
    if n == 0 {
        String::new()
    } else {
        format!("\x1b[{n}{direction}")
    }
}
pub fn cursor_move(x: i64, y: i64) -> String {
    let mut result = String::new();
    if x < 0 {
        result.push_str(&cursor_motion('D', -x));
    }
    if x > 0 {
        result.push_str(&cursor_motion('C', x));
    }
    if y < 0 {
        result.push_str(&cursor_motion('A', -y));
    }
    if y > 0 {
        result.push_str(&cursor_motion('B', y));
    }
    result
}
pub fn erase_lines(n: i64) -> String {
    if n <= 0 {
        return String::new();
    }
    // 2.1.88 emits one erase and a cursor-up per erased line, ending at col 1.
    let mut result = String::new();
    for i in 0..n {
        result.push_str("\x1b[2K");
        if i + 1 < n {
            result.push_str("\x1b[1A");
        }
    }
    result.push_str("\x1b[G");
    result
}
fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Array(xs) => xs
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
        _ => value.to_string(),
    }
}

// src/utils/ansiToSvg.ts: exact 16/256/true-colour palettes, reset logic,
// paragraph segmentation, SVG tspan structure and per-call rendering options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: i64,
    pub g: i64,
    pub b: i64,
}
const FG: Rgb = Rgb {
    r: 229,
    g: 229,
    b: 229,
};
const BG: Rgb = Rgb {
    r: 30,
    g: 30,
    b: 30,
};
const PALETTE: [Rgb; 16] = [
    Rgb { r: 0, g: 0, b: 0 },
    Rgb {
        r: 205,
        g: 49,
        b: 49,
    },
    Rgb {
        r: 13,
        g: 188,
        b: 121,
    },
    Rgb {
        r: 229,
        g: 229,
        b: 16,
    },
    Rgb {
        r: 36,
        g: 114,
        b: 200,
    },
    Rgb {
        r: 188,
        g: 63,
        b: 188,
    },
    Rgb {
        r: 17,
        g: 168,
        b: 205,
    },
    Rgb {
        r: 229,
        g: 229,
        b: 229,
    },
    Rgb {
        r: 102,
        g: 102,
        b: 102,
    },
    Rgb {
        r: 241,
        g: 76,
        b: 76,
    },
    Rgb {
        r: 35,
        g: 209,
        b: 139,
    },
    Rgb {
        r: 245,
        g: 245,
        b: 67,
    },
    Rgb {
        r: 59,
        g: 142,
        b: 234,
    },
    Rgb {
        r: 214,
        g: 112,
        b: 214,
    },
    Rgb {
        r: 41,
        g: 184,
        b: 219,
    },
    Rgb {
        r: 255,
        g: 255,
        b: 255,
    },
];
const STANDARD256: [Rgb; 16] = [
    Rgb { r: 0, g: 0, b: 0 },
    Rgb { r: 128, g: 0, b: 0 },
    Rgb { r: 0, g: 128, b: 0 },
    Rgb {
        r: 128,
        g: 128,
        b: 0,
    },
    Rgb { r: 0, g: 0, b: 128 },
    Rgb {
        r: 128,
        g: 0,
        b: 128,
    },
    Rgb {
        r: 0,
        g: 128,
        b: 128,
    },
    Rgb {
        r: 192,
        g: 192,
        b: 192,
    },
    Rgb {
        r: 128,
        g: 128,
        b: 128,
    },
    Rgb { r: 255, g: 0, b: 0 },
    Rgb { r: 0, g: 255, b: 0 },
    Rgb {
        r: 255,
        g: 255,
        b: 0,
    },
    Rgb { r: 0, g: 0, b: 255 },
    Rgb {
        r: 255,
        g: 0,
        b: 255,
    },
    Rgb {
        r: 0,
        g: 255,
        b: 255,
    },
    Rgb {
        r: 255,
        g: 255,
        b: 255,
    },
];
fn color256(index: i64) -> Rgb {
    if (0..16).contains(&index) {
        return STANDARD256[index as usize];
    }
    if index < 232 {
        let i = index - 16;
        let channel = |x| if x == 0 { 0 } else { 55 + 40 * x };
        return Rgb {
            r: channel(i.div_euclid(36)),
            g: channel(i.rem_euclid(36).div_euclid(6)),
            b: channel(i.rem_euclid(6)),
        };
    }
    let gray = (index - 232) * 10 + 8;
    Rgb {
        r: gray,
        g: gray,
        b: gray,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    pub color: Rgb,
    pub bold: bool,
}
fn push_span(spans: &mut Vec<TextSpan>, text: &str, color: Rgb, bold: bool) {
    if !text.is_empty() {
        spans.push(TextSpan {
            text: text.into(),
            color,
            bold,
        });
    }
}
pub fn parse_ansi(text: &str) -> Vec<Vec<TextSpan>> {
    text.split('\n')
        .map(|line| {
            let mut spans = Vec::new();
            let mut color = FG;
            let mut bold = false;
            let mut i = 0usize;
            let bytes = line.as_bytes();
            while i < bytes.len() {
                if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'[') {
                    let mut j = i + 2;
                    while j < bytes.len() && !bytes[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    if bytes.get(j) == Some(&b'm') {
                        let codes = line[i + 2..j]
                            .split(';')
                            .map(|v| v.parse::<i64>().unwrap_or(0))
                            .collect::<Vec<_>>();
                        let mut k = 0;
                        while k < codes.len() {
                            match codes[k] {
                                0 => {
                                    color = FG;
                                    bold = false;
                                }
                                1 => bold = true,
                                30..=37 => color = PALETTE[(codes[k] - 30) as usize],
                                90..=97 => color = PALETTE[(codes[k] - 90 + 8) as usize],
                                39 => color = FG,
                                38 if codes.get(k + 1) == Some(&5)
                                    && codes.get(k + 2).is_some() =>
                                {
                                    color = color256(codes[k + 2]);
                                    k += 2;
                                }
                                38 if codes.get(k + 1) == Some(&2)
                                    && codes.get(k + 4).is_some() =>
                                {
                                    color = Rgb {
                                        r: codes[k + 2],
                                        g: codes[k + 3],
                                        b: codes[k + 4],
                                    };
                                    k += 4;
                                }
                                _ => {}
                            }
                            k += 1;
                        }
                    }
                    i = (j + 1).min(bytes.len());
                    continue;
                }
                let start = i;
                while i < bytes.len() && bytes[i] != 0x1b {
                    i += 1;
                }
                if start < i {
                    push_span(&mut spans, &line[start..i], color, bold);
                } else {
                    // Match original parser's progress on a dangling ESC without '['.
                    i += 1;
                }
            }
            if spans.is_empty() {
                spans.push(TextSpan {
                    text: String::new(),
                    color: FG,
                    bold: false,
                });
            }
            spans
        })
        .collect()
}
fn escape_xml(src: &str) -> String {
    src.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn option_str<'a>(options: &'a Value, key: &str, default: &'a str) -> &'a str {
    options.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn option_num(options: &Value, key: &str, default: f64) -> f64 {
    options.get(key).and_then(Value::as_f64).unwrap_or(default)
}
fn number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
}
pub fn ansi_to_svg(ansi_text: &str, options: &Value) -> String {
    let font = option_str(options, "fontFamily", "Menlo, Monaco, monospace");
    let size = option_num(options, "fontSize", 14.0);
    let line_height = option_num(options, "lineHeight", 22.0);
    let px = option_num(options, "paddingX", 24.0);
    let py = option_num(options, "paddingY", 24.0);
    let bg = format!("rgb({}, {}, {})", BG.r, BG.g, BG.b);
    let background = option_str(options, "backgroundColor", &bg);
    let radius = option_num(options, "borderRadius", 8.0);
    let mut lines = parse_ansi(ansi_text);
    while lines
        .last()
        .is_some_and(|row| row.iter().all(|span| span.text.trim().is_empty()))
    {
        lines.pop();
    }
    let max_len = lines
        .iter()
        .map(|row| {
            row.iter()
                .map(|span| span.text.encode_utf16().count())
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0);
    let width = (max_len as f64 * size * 0.6 + px * 2.0).ceil();
    let height = (lines.len() as f64) * line_height + py * 2.0;
    let (width, height) = (number(width), number(height));
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    svg.push_str(&format!(
        "  <rect width=\"100%\" height=\"100%\" fill=\"{background}\" rx=\"{}\" ry=\"{}\"/>\n",
        number(radius),
        number(radius)
    ));
    svg.push_str(&format!("  <style>\n    text {{ font-family: {font}; font-size: {}px; white-space: pre; }}\n    .b {{ font-weight: bold; }}\n  </style>\n",number(size)));
    for (i, row) in lines.iter().enumerate() {
        let y = py + (i as f64 + 1.0) * line_height - (line_height - size) / 2.0;
        svg.push_str(&format!(
            "  <text x=\"{}\" y=\"{}\" xml:space=\"preserve\">",
            number(px),
            number(y)
        ));
        for span in row {
            if span.text.is_empty() {
                continue;
            }
            let cls = if span.bold { " class=\"b\"" } else { "" };
            svg.push_str(&format!(
                "<tspan fill=\"rgb({}, {}, {})\"{cls}>{}</tspan>",
                span.color.r,
                span.color.g,
                span.color.b,
                escape_xml(&span.text)
            ));
        }
        svg.push_str("</text>\n");
    }
    svg.push_str("</svg>");
    svg
}

// src/utils/streamlinedTransform.ts
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ToolCounts {
    searches: u32,
    reads: u32,
    writes: u32,
    commands: u32,
    other: u32,
}
impl ToolCounts {
    fn accumulate(&mut self, name: &str) {
        let starts = |labels: &[&str]| labels.iter().any(|prefix| name.starts_with(prefix));
        if starts(&["Grep", "Glob", "WebSearch", "LSP"]) {
            self.searches += 1;
        } else if starts(&["Read", "ListMcpResources"]) {
            self.reads += 1;
        } else if starts(&["Write", "Edit", "NotebookEdit"]) {
            self.writes += 1;
        } else if starts(&["Bash", "PowerShell", "Tmux", "TaskStop"]) {
            self.commands += 1;
        } else {
            self.other += 1;
        }
    }
    fn summary(&self) -> Option<String> {
        let mut parts = Vec::new();
        let entries = [
            (self.searches, "searched", "pattern", "patterns"),
            (self.reads, "read", "file", "files"),
            (self.writes, "wrote", "file", "files"),
            (self.commands, "ran", "command", "commands"),
        ];
        for (n, verb, single, plural) in entries {
            if n > 0 {
                parts.push(format!(
                    "{verb} {n} {}",
                    if n == 1 { single } else { plural }
                ));
            }
        }
        if self.other > 0 {
            parts.push(format!(
                "{} other {}",
                self.other,
                if self.other == 1 { "tool" } else { "tools" }
            ));
        }
        if parts.is_empty() {
            return None;
        }
        let mut result = parts.join(", ");
        if let Some(first) = result.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        Some(result)
    }
}
#[derive(Debug, Default)]
pub struct StreamlinedTransformer {
    counts: ToolCounts,
}
pub fn should_include_streamlined(msg: &Value) -> bool {
    matches!(
        msg.get("type").and_then(Value::as_str),
        Some("assistant" | "result")
    )
}
impl StreamlinedTransformer {
    pub fn transform(&mut self, message: &Value) -> Option<Value> {
        match message.get("type")?.as_str()? {
            "assistant" => {
                let content = message
                    .pointer("/message/content")
                    .and_then(Value::as_array);
                let mut text = Vec::new();
                if let Some(blocks) = content {
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) == Some("text") {
                            if let Some(t) = block.get("text").and_then(Value::as_str) {
                                text.push(t);
                            }
                        }
                        if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                            if let Some(name) = block.get("name").and_then(Value::as_str) {
                                self.counts.accumulate(name);
                            }
                        }
                    }
                }
                let text = text.join("\n").trim().to_owned();
                let mut result = if !text.is_empty() {
                    self.counts = ToolCounts::default();
                    json!({"type":"streamlined_text","text":text})
                } else {
                    json!({"type":"streamlined_tool_use_summary","tool_summary":self.counts.summary()?})
                };
                if let Some(session) = message.get("session_id") {
                    result["session_id"] = session.clone();
                }
                if let Some(uuid) = message.get("uuid") {
                    result["uuid"] = uuid.clone();
                }
                Some(result)
            }
            "result" => Some(message.clone()),
            _ => None,
        }
    }
}

/// Dispatch confirmed native Rust semantics for incomplete AST-generated exports.
/// Unimplemented exports intentionally fall through to the separate partial registry.
pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    let a = args.first().unwrap_or(&Value::Null);
    let n = |v: &Value| v.as_i64().unwrap_or(0);
    fn s(v: &Value) -> &str {
        v.as_str().unwrap_or("")
    }
    let arg = |i: usize| args.get(i).unwrap_or(&Value::Null);
    match (source, export) {
        ("src/ink/termio/ansi.ts", "isC0") => Some(json!(is_c0(n(a)))),
        ("src/ink/termio/ansi.ts", "isEscFinal") => Some(json!(is_esc_final(n(a)))),
        ("src/ink/termio/csi.ts", "isCSIParam") => Some(json!(is_csi_param(n(a)))),
        ("src/ink/termio/csi.ts", "isCSIIntermediate") => Some(json!(is_csi_intermediate(n(a)))),
        ("src/ink/termio/csi.ts", "isCSIFinal") => Some(json!(is_csi_final(n(a)))),
        ("src/ink/termio/csi.ts", "csi") => Some(json!(csi(args))),
        ("src/ink/termio/csi.ts", "cursorUp") => Some(json!(cursor_motion(
            'A',
            if args.is_empty() { 1 } else { n(a) }
        ))),
        ("src/ink/termio/csi.ts", "cursorDown") => Some(json!(cursor_motion(
            'B',
            if args.is_empty() { 1 } else { n(a) }
        ))),
        ("src/ink/termio/csi.ts", "cursorForward") => Some(json!(cursor_motion(
            'C',
            if args.is_empty() { 1 } else { n(a) }
        ))),
        ("src/ink/termio/csi.ts", "cursorBack") => Some(json!(cursor_motion(
            'D',
            if args.is_empty() { 1 } else { n(a) }
        ))),
        ("src/ink/termio/csi.ts", "cursorMove") => Some(json!(cursor_move(n(a), n(arg(1))))),
        ("src/ink/termio/csi.ts", "cursorTo") => Some(json!(csi(&[a.clone(), json!("G")]))),
        ("src/ink/termio/csi.ts", "cursorPosition") => {
            Some(json!(csi(&[a.clone(), arg(1).clone(), json!("H")])))
        }
        ("src/ink/termio/csi.ts", "eraseLines") => Some(json!(erase_lines(n(a)))),
        ("src/ink/termio/csi.ts", "scrollUp") => Some(json!(cursor_motion(
            'S',
            if args.is_empty() { 1 } else { n(a) }
        ))),
        ("src/ink/termio/csi.ts", "scrollDown") => Some(json!(cursor_motion(
            'T',
            if args.is_empty() { 1 } else { n(a) }
        ))),
        ("src/ink/termio/csi.ts", "setScrollRegion") => {
            Some(json!(csi(&[a.clone(), arg(1).clone(), json!("r")])))
        }
        ("src/ink/termio/csi.ts", "eraseToEndOfLine") => Some(json!(csi(&[json!("K")]))),
        ("src/ink/termio/csi.ts", "eraseToStartOfLine") => {
            Some(json!(csi(&[json!(1), json!("K")])))
        }
        ("src/ink/termio/csi.ts", "eraseLine") => Some(json!(csi(&[json!(2), json!("K")]))),
        ("src/ink/termio/csi.ts", "eraseToEndOfScreen") => Some(json!(csi(&[json!("J")]))),
        ("src/ink/termio/csi.ts", "eraseToStartOfScreen") => {
            Some(json!(csi(&[json!(1), json!("J")])))
        }
        ("src/ink/termio/csi.ts", "eraseScreen") => Some(json!(csi(&[json!(2), json!("J")]))),
        ("src/ink/termio/dec.ts", "decset") => Some(json!(decset(n(a)))),
        ("src/ink/termio/dec.ts", "decreset") => Some(json!(decreset(n(a)))),
        ("src/utils/ansiToSvg.ts", "parseAnsi") => Some(json!(parse_ansi(s(a)))),
        ("src/utils/ansiToSvg.ts", "ansiToSvg") => Some(json!(ansi_to_svg(s(a), arg(1)))),
        ("src/utils/streamlinedTransform.ts", "shouldIncludeInStreamlined") => {
            Some(json!(should_include_streamlined(a)))
        }
        ("src/utils/streamlinedTransform.ts", "transformBatch") => {
            let mut t = StreamlinedTransformer::default();
            Some(Value::Array(
                a.as_array()?
                    .iter()
                    .filter_map(|x| t.transform(x))
                    .collect(),
            ))
        }
        _ => None,
    }
}
pub fn exports(source: &str) -> Option<&'static [&'static str]> {
    match source {
        "src/ink/termio/ansi.ts" => Some(&["isC0", "isEscFinal"]),
        "src/ink/termio/csi.ts" => Some(&[
            "isCSIParam",
            "isCSIIntermediate",
            "isCSIFinal",
            "csi",
            "cursorUp",
            "cursorDown",
            "cursorForward",
            "cursorBack",
            "cursorTo",
            "cursorPosition",
            "cursorMove",
            "eraseToEndOfLine",
            "eraseToStartOfLine",
            "eraseLine",
            "eraseToEndOfScreen",
            "eraseToStartOfScreen",
            "eraseScreen",
            "eraseLines",
            "scrollUp",
            "scrollDown",
            "setScrollRegion",
        ]),
        "src/ink/termio/dec.ts" => Some(&["decset", "decreset"]),
        "src/utils/ansiToSvg.ts" => Some(&["parseAnsi", "ansiToSvg"]),
        "src/utils/streamlinedTransform.ts" => {
            Some(&["shouldIncludeInStreamlined", "transformBatch"])
        }
        _ => None,
    }
}
#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn csi_and_dec_are_original_sequences() {
        assert_eq!(csi(&[]), "\x1b[");
        assert_eq!(csi(&[json!(2), json!(3), json!("H")]), "\x1b[2;3H");
        assert_eq!(decset(2004), "\x1b[?2004h");
        assert_eq!(decreset(1006), "\x1b[?1006l");
        assert_eq!(cursor_move(-2, 3), "\x1b[2D\x1b[3B");
        assert_eq!(erase_lines(2), "\x1b[2K\x1b[1A\x1b[2K\x1b[G");
        assert!(is_c0(0));
        assert!(is_esc_final(0x40));
        assert!(!is_csi_final(0x39));
    }
    #[test]
    fn ansi_split_palette_and_reset() {
        let rows = parse_ansi("a\x1b[1;31mb\x1b[38;5;196mc\x1b[38;2;1;2;3md\x1b[0me\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 5);
        assert_eq!(
            rows[0][1],
            TextSpan {
                text: "b".into(),
                color: PALETTE[1],
                bold: true
            }
        );
        assert_eq!(rows[0][2].color, Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(rows[0][3].color, Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(
            rows[0][4],
            TextSpan {
                text: "e".into(),
                color: FG,
                bold: false
            }
        );
        assert_eq!(rows[1][0].text, "");
    }
    #[test]
    fn svg_escapes_input_and_trims_blank_lines() {
        let svg = ansi_to_svg("a<&\x1b[1;31mb\x1b[0m\n\n", &Value::Null);
        assert!(svg.contains("a&lt;&amp;"));
        assert!(svg.contains("class=\"b\""));
        assert!(svg.contains("width=\"82\""));
        assert!(svg.contains("height=\"70\""));
        assert_eq!(svg.matches("<text ").count(), 1);
    }
    #[test]
    fn stateful_sdk_tool_accumulation_and_reset() {
        let mut t = StreamlinedTransformer::default();
        let a = json!({"type":"assistant","session_id":"sid","uuid":"1","message":{"content":[{"type":"tool_use","name":"Grep"},{"type":"tool_use","name":"Bash"}]}});
        let b = json!({"type":"assistant","session_id":"sid","uuid":"2","message":{"content":[{"type":"tool_use","name":"Read"}]}});
        let c = json!({"type":"assistant","session_id":"sid","uuid":"3","message":{"content":[{"type":"thinking","thinking":"hidden"},{"type":"text","text":" Done "}]}});
        assert_eq!(
            t.transform(&a).unwrap()["tool_summary"],
            "Searched 1 pattern, ran 1 command"
        );
        assert_eq!(
            t.transform(&b).unwrap()["tool_summary"],
            "Searched 1 pattern, read 1 file, ran 1 command"
        );
        assert_eq!(t.transform(&c).unwrap()["text"], "Done");
        assert!(t.transform(&json!({"type":"system"})).is_none());
        assert_eq!(
            t.transform(&json!({"type":"result","permission_denials":[1]}))
                .unwrap()["permission_denials"],
            json!([1])
        );
        assert!(t
            .transform(&json!({"type":"assistant","message":{"content":[]}}))
            .is_none());
        assert!(should_include_streamlined(&a));
    }
}
