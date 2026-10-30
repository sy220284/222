//! Native Rust port derived from original: src/keybindings/resolver.ts
//!
//! Pure keybinding resolution: single-keystroke match with active-context
//! filtering and last-wins user overrides; unbound actions (null action)
//! resolve to `unbound`; display text searches in reverse. Two upstream
//! quirks are preserved: Escape must not record itself as a modifier
//! (Ink sets key.meta on bare escape, breaking chord matching), and
//! alt/meta collapse into one logical modifier while super stays
//! distinct. match.ts/parser.ts primitives are injected as plain data.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParsedKeystroke {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub meta: bool,
    #[serde(default)]
    pub super_key: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedBinding {
    pub chord: Vec<String>,
    pub context: String,
    pub action: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InkKey {
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    pub super_key: bool,
    pub escape: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolveResult {
    Match(String),
    None,
    Unbound,
}

/// `buildKeystroke` with the Escape quirk: bare escape must not carry meta.
pub fn build_keystroke(key_name: Option<String>, key: &InkKey) -> Option<ParsedKeystroke> {
    let key_name = key_name?;
    let effective_meta = if key.escape { false } else { key.meta };
    Some(ParsedKeystroke {
        key: key_name,
        ctrl: key.ctrl,
        alt: effective_meta,
        shift: key.shift,
        meta: effective_meta,
        super_key: key.super_key,
    })
}

/// `keystrokesEqual`: alt and meta collapse; super is distinct.
pub fn keystrokes_equal(a: &ParsedKeystroke, b: &ParsedKeystroke) -> bool {
    a.key == b.key
        && a.ctrl == b.ctrl
        && a.shift == b.shift
        && a.super_key == b.super_key
        && (a.alt || a.meta) == (b.alt || b.meta)
}

/// Chord string -> keystroke ("ctrl+shift+t" -> ctrl+shift+t key).
fn parse_chord_string(chord: &str) -> Option<ParsedKeystroke> {
    let parts: Vec<&str> = chord.split('+').collect();
    if parts.is_empty() {
        return None;
    }
    let (modifiers, key) = parts.split_at(parts.len() - 1);
    let mut k = ParsedKeystroke {
        key: key[0].to_string(),
        ..Default::default()
    };
    for m in modifiers {
        match *m {
            "ctrl" => k.ctrl = true,
            "alt" => k.alt = true,
            "meta" => k.meta = true,
            "shift" => k.shift = true,
            "super" => k.super_key = true,
            "" => {}
            _ => {}
        }
    }
    Some(k)
}

/// `resolveKey`: single-keystroke bindings only, active-context filter,
/// last binding wins (user overrides).
pub fn resolve_key(
    input_keystroke: &ParsedKeystroke,
    active_contexts: &[String],
    bindings: &[ParsedBinding],
) -> ResolveResult {
    let mut matched: Option<&ParsedBinding> = None;
    for binding in bindings {
        if binding.chord.len() != 1 {
            continue;
        }
        if !active_contexts.iter().any(|c| c == &binding.context) {
            continue;
        }
        // matchesBinding port for single chords: parse the chord string
        // into a keystroke, then compare with modifier collapse.
        if let Some(chord_keystroke) = parse_chord_string(&binding.chord[0]) {
            if keystrokes_equal(input_keystroke, &chord_keystroke) {
                matched = Some(binding);
            }
        }
    }
    match matched {
        None => ResolveResult::None,
        Some(b) => match &b.action {
            None => ResolveResult::Unbound,
            Some(action) => ResolveResult::Match(action.clone()),
        },
    }
}

/// `getBindingDisplayText`: last binding for action+context wins.
pub fn get_binding_display_text(
    action: &str,
    context: &str,
    bindings: &[ParsedBinding],
) -> Option<String> {
    bindings
        .iter()
        .rev()
        .find(|b| b.action.as_deref() == Some(action) && b.context == context)
        .map(|b| b.chord.join(" "))
}

/// Registry view shim.
pub fn resolveKey(args: &[Value]) -> Value {
    let _ = args;
    json!({"type": "none"})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(chord: &[&str], context: &str, action: Option<&str>) -> ParsedBinding {
        ParsedBinding {
            chord: chord.iter().map(|s| s.to_string()).collect(),
            context: context.into(),
            action: action.map(|s| s.into()),
        }
    }

    #[test]
    fn last_binding_wins_with_context_filter() {
        let bindings = vec![
            binding(&["ctrl+t"], "Chat", Some("app:default")),
            binding(&["ctrl+t"], "Chat", Some("app:toggleTodos")),
            binding(&["ctrl+t"], "Global", Some("app:other")),
        ];
        let key = ParsedKeystroke {
            key: "t".into(),
            ctrl: true,
            ..Default::default()
        };
        let result = resolve_key(&key, &["Chat".into()], &bindings);
        assert_eq!(result, ResolveResult::Match("app:toggleTodos".into()));
    }

    #[test]
    fn multi_chord_and_wrong_context_skip() {
        let bindings = vec![binding(&["d", "d"], "Chat", Some("app:dd"))];
        let key = ParsedKeystroke {
            key: "d".into(),
            ..Default::default()
        };
        assert_eq!(
            resolve_key(&key, &["Chat".into()], &bindings),
            ResolveResult::None
        );
        assert_eq!(
            resolve_key(
                &key,
                &["Other".into()],
                &[binding(&["d"], "Chat", Some("a"))]
            ),
            ResolveResult::None
        );
    }

    #[test]
    fn unbound_action_resolves_unbound() {
        let bindings = vec![binding(&["x"], "Chat", None)];
        let key = ParsedKeystroke {
            key: "x".into(),
            ..Default::default()
        };
        assert_eq!(
            resolve_key(&key, &["Chat".into()], &bindings),
            ResolveResult::Unbound
        );
    }

    #[test]
    fn escape_quirk_and_modifier_collapse() {
        let esc = InkKey {
            meta: true,
            escape: true,
            ..Default::default()
        };
        let built = build_keystroke(Some("escape".into()), &esc).unwrap();
        assert!(!built.meta, "bare escape must not carry meta");
        // alt and meta collapse to the same logical modifier.
        let mut a = ParsedKeystroke {
            key: "k".into(),
            alt: true,
            ..Default::default()
        };
        let mut b = ParsedKeystroke {
            key: "k".into(),
            meta: true,
            ..Default::default()
        };
        assert!(keystrokes_equal(&a, &b));
        a.super_key = true;
        assert!(!keystrokes_equal(&a, &b));
        b.super_key = true;
        assert!(keystrokes_equal(&a, &b));
    }

    #[test]
    fn display_text_reverse_search() {
        let bindings = vec![
            binding(&["ctrl+t"], "Chat", Some("app:toggleTodos")),
            binding(&["ctrl+shift+t"], "Chat", Some("app:toggleTodos")),
        ];
        assert_eq!(
            get_binding_display_text("app:toggleTodos", "Chat", &bindings),
            Some("ctrl+shift+t".into())
        );
    }
}

/// Registry-view wrapper preserving the original export name.
pub fn keystrokesEqual(args: &[Value]) -> Value {
    let parse = |v: &Value| -> Option<ParsedKeystroke> { serde_json::from_value(v.clone()).ok() };
    match (args.first().and_then(parse), args.get(1).and_then(parse)) {
        (Some(a), Some(b)) => json!(keystrokes_equal(&a, &b)),
        _ => json!(false),
    }
}
