//! Rust translation of the portable OSC sequence, colour and tab-status
//! state machine from the 2.1.88 terminal source. Vendor-bound notification
//! endpoints and clipboard subprocesses are deliberately not recreated here.
#![allow(dead_code)]
use serde_json::{json, Map, Value};

/// src/ink/termio/osc.ts: OSC separator and terminal-dependent terminator.
pub fn osc(parts: &[Value], kitty: bool) -> String {
    let data = parts
        .iter()
        .map(crate::generated_ports::runtime::to_string)
        .collect::<Vec<_>>()
        .join(";");
    format!("\x1b]{data}{}", if kitty { "\x1b\\" } else { "\x07" })
}
pub fn wrap_for_multiplexer(sequence: &str, tmux: bool, screen: bool) -> String {
    if tmux {
        return format!("\x1bPtmux;{}\x1b\\", sequence.replace('\x1b', "\x1b\x1b"));
    }
    if screen {
        return format!("\x1bP{sequence}\x1b\\");
    }
    sequence.into()
}
pub fn clipboard_path(darwin: bool, ssh_connection: bool, tmux: bool) -> &'static str {
    if darwin && !ssh_connection {
        "native"
    } else if tmux {
        "tmux-buffer"
    } else {
        "osc52"
    }
}

pub fn parse_osc_color(spec: &str) -> Option<Value> {
    let rgb = if let Some(hex) = spec.strip_prefix('#') {
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let component = |a: usize, b: usize| i64::from_str_radix(&hex[a..b], 16).ok();
        [component(0, 2)?, component(2, 4)?, component(4, 6)?]
    } else if let Some(rest) = spec.strip_prefix("rgb:") {
        let xs = rest.split('/').collect::<Vec<_>>();
        if xs.len() != 3
            || xs
                .iter()
                .any(|v| v.is_empty() || v.len() > 4 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return None;
        }
        let convert = |part: &str| {
            let num = i64::from_str_radix(part, 16).ok()?;
            let denom = (16_i64.pow(part.len() as u32) - 1) as f64;
            Some((num as f64 / denom * 255.).round() as i64)
        };
        [convert(xs[0])?, convert(xs[1])?, convert(xs[2])?]
    } else {
        return None;
    };
    Some(json!({"type":"rgb","r":rgb[0],"g":rgb[1],"b":rgb[2]}))
}
fn split_tab_status_pairs(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut key = String::new();
    let mut val = String::new();
    let mut in_val = false;
    let mut esc = false;
    for c in text.chars() {
        if esc {
            if in_val {
                val.push(c)
            } else {
                key.push(c)
            }
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ';' {
            result.push((std::mem::take(&mut key), std::mem::take(&mut val)));
            in_val = false;
        } else if c == '=' && !in_val {
            in_val = true;
        } else if in_val {
            val.push(c)
        } else {
            key.push(c)
        }
    }
    if !key.is_empty() || in_val {
        result.push((key, val));
    }
    result
}
fn parse_tab_status(data: &str) -> Value {
    let mut obj = Map::new();
    for (key, val) in split_tab_status_pairs(data) {
        match key.as_str() {
            "indicator" => {
                obj.insert(
                    "indicator".into(),
                    if val.is_empty() {
                        Value::Null
                    } else {
                        parse_osc_color(&val).unwrap_or(Value::Null)
                    },
                );
            }
            "status" => {
                obj.insert(
                    "status".into(),
                    if val.is_empty() {
                        Value::Null
                    } else {
                        json!(val)
                    },
                );
            }
            "status-color" => {
                obj.insert(
                    "statusColor".into(),
                    if val.is_empty() {
                        Value::Null
                    } else {
                        parse_osc_color(&val).unwrap_or(Value::Null)
                    },
                );
            }
            _ => {}
        }
    }
    Value::Object(obj)
}
pub fn parse_osc(content: &str) -> Value {
    let (cmd, data) = content.split_once(';').unwrap_or((content, ""));
    // JavaScript parseInt accepts valid leading integer prefixes.
    let digits = cmd
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    let number = digits.parse::<u32>().ok();
    match number {
        Some(0) => json!({"type":"title","action":{"type":"both","title":data}}),
        Some(1) => json!({"type":"title","action":{"type":"iconName","name":data}}),
        Some(2) => json!({"type":"title","action":{"type":"windowTitle","title":data}}),
        Some(8) => {
            let (raw_params, url) = data.split_once(';').unwrap_or((data, ""));
            if url.is_empty() {
                return json!({"type":"link","action":{"type":"end"}});
            }
            let mut params = Map::new();
            for pair in raw_params.split(':') {
                if let Some((key, val)) = pair.split_once('=') {
                    params.insert(key.to_owned(), json!(val));
                }
            }
            let mut action = json!({"type":"start","url":url});
            if !params.is_empty() {
                action["params"] = Value::Object(params)
            }
            json!({"type":"link","action":action})
        }
        Some(21337) => json!({"type":"tabStatus","action":parse_tab_status(data)}),
        _ => json!({"type":"unknown","sequence":format!("\x1b]{content}")}),
    }
}
fn hash_url_id(url: &str) -> String {
    let mut value: i32 = 0;
    for ch in url.encode_utf16() {
        value = value.wrapping_mul(31).wrapping_add(ch as i32)
    }
    let mut n = value as u32;
    if n == 0 {
        return "0".into();
    }
    let mut rev = Vec::new();
    while n > 0 {
        let d = (n % 36) as u8;
        rev.push(if d < 10 { b'0' + d } else { b'a' + d - 10 });
        n /= 36;
    }
    rev.reverse();
    String::from_utf8(rev).expect("ASCII base36")
}
pub fn osc_link(url: &str, params: &Value, kitty: bool) -> String {
    if url.is_empty() {
        return osc(&[json!(8), json!(""), json!("")], kitty);
    }
    let mut entries = vec![("id".to_owned(), hash_url_id(url))];
    if let Some(map) = params.as_object() {
        for (k, v) in map {
            if let Some(item) = entries.iter_mut().find(|x| &x.0 == k) {
                item.1 = crate::generated_ports::runtime::to_string(v)
            } else {
                entries.push((k.clone(), crate::generated_ports::runtime::to_string(v)))
            }
        }
    }
    let parts = entries
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(":");
    osc(&[json!(8), json!(parts), json!(url)], kitty)
}
fn rgb_color(color: &Value) -> String {
    if color.get("type").and_then(Value::as_str) != Some("rgb") {
        return String::new();
    }
    let component = |key| {
        format!(
            "{:02x}",
            color.get(key).and_then(Value::as_i64).unwrap_or(0)
        )
    };
    format!("#{}{}{}", component("r"), component("g"), component("b"))
}
pub fn osc_tab_status(fields: &Value, kitty: bool) -> String {
    let mut parts = Vec::new();
    if let Some(m) = fields.as_object() {
        if let Some(color) = m.get("indicator") {
            parts.push(format!("indicator={}", rgb_color(color)))
        }
        if let Some(status) = m.get("status") {
            parts.push(format!(
                "status={}",
                status
                    .as_str()
                    .unwrap_or("")
                    .replace('\\', "\\\\")
                    .replace(';', "\\;")
            ));
        }
        if let Some(color) = m.get("statusColor") {
            parts.push(format!("status-color={}", rgb_color(color)))
        }
    }
    osc(&[json!(21337), json!(parts.join(";"))], kitty)
}
/// Actual Rust exported entrypoints for the prior static-only OSC module.
pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    if source != "src/ink/termio/osc.ts" {
        return None;
    }
    let a = args.first().unwrap_or(&Value::Null);
    let b = args.get(1).unwrap_or(&Value::Null);
    fn s(v: &Value) -> &str {
        v.as_str().unwrap_or("")
    }
    let kitty = std::env::var("TERM_PROGRAM").is_ok_and(|x| x.eq_ignore_ascii_case("kitty"));
    match export {
        "osc" => Some(json!(osc(args, kitty))),
        "wrapForMultiplexer" => Some(json!(wrap_for_multiplexer(
            s(a),
            std::env::var_os("TMUX").is_some(),
            std::env::var_os("STY").is_some()
        ))),
        "getClipboardPath" => Some(json!(clipboard_path(
            cfg!(target_os = "macos"),
            std::env::var_os("SSH_CONNECTION").is_some(),
            std::env::var_os("TMUX").is_some()
        ))),
        "parseOSC" => Some(parse_osc(s(a))),
        "parseOscColor" => Some(parse_osc_color(s(a)).unwrap_or(Value::Null)),
        "link" => Some(json!(osc_link(s(a), b, kitty))),
        "tabStatus" => Some(json!(osc_tab_status(a, kitty))),
        _ => None,
    }
}
pub fn exports(source: &str) -> Option<&'static [&'static str]> {
    if source == "src/ink/termio/osc.ts" {
        Some(&[
            "osc",
            "wrapForMultiplexer",
            "getClipboardPath",
            "parseOSC",
            "parseOscColor",
            "link",
            "tabStatus",
        ])
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colour_spec_and_osc_actions() {
        assert_eq!(
            parse_osc_color("#ff8000").unwrap(),
            json!({"type":"rgb","r":255,"g":128,"b":0})
        );
        assert_eq!(
            parse_osc_color("rgb:f/8/0").unwrap(),
            json!({"type":"rgb","r":255,"g":136,"b":0})
        );
        assert!(parse_osc_color("#red").is_none());
        assert_eq!(
            parse_osc("0;Hello")["action"],
            json!({"type":"both","title":"Hello"})
        );
        assert_eq!(parse_osc("8;;")["action"], json!({"type":"end"}));
        assert_eq!(
            parse_osc("8;id=foo;https://example.com/a;b")["action"]["url"],
            "https://example.com/a;b"
        );
        assert_eq!(
            parse_osc("21337;status=a\\;b;status-color=#abcdef")["action"]["status"],
            "a;b"
        );
    }
    #[test]
    fn url_ids_and_wrapping() {
        assert_eq!(osc(&[json!(0), json!("hey")], false), "\x1b]0;hey\x07");
        assert_eq!(osc(&[json!(0)], true), "\x1b]0\x1b\\");
        assert_eq!(
            wrap_for_multiplexer("\x1b]0;hi\x07", true, false),
            "\x1bPtmux;\x1b\x1b]0;hi\x07\x1b\\"
        );
        assert_eq!(clipboard_path(true, false, true), "native");
        assert_eq!(clipboard_path(false, true, true), "tmux-buffer");
        assert!(osc_link("https://e.test", &json!({}), false).starts_with("\x1b]8;id="));
        assert_eq!(
            osc_tab_status(
                &json!({"status":"a;b\\c","indicator":{"type":"rgb","r":255,"g":0,"b":0}}),
                false
            ),
            "\x1b]21337;indicator=#ff0000;status=a\\;b\\\\c\x07"
        );
    }
}
