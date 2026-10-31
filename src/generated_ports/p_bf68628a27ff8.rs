//! Native Rust port derived from original: src/buddy/prompt.ts
//!
//! Companion intro prompt text and the attachment dedup gate. The
//! compile-time `BUDDY` feature maps to a process-lifetime switch
//! (default on, matching a production build); the muted check and the
//! "already announced for this companion" scan mirror the original.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};

pub static BUDDY_FEATURE_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn companion_intro_text(name: &str, species: &str) -> String {
    format!(
        "# Companion\n\nA small {species} named {name} sits beside the user's input box and \
occasionally comments in a speech bubble. You're not {name} — it's a separate watcher.\n\n\
When the user addresses {name} directly (by name), its bubble will answer. Your job in that \
moment is to stay out of the way: respond in ONE line or less, or just answer any part of \
the message meant for you. Don't explain that you're not {name} — they know. Don't narrate \
what {name} might say — the bubble handles that."
    )
}

/// Attachment payload for a companion introduction.
pub fn companion_intro_attachment(companion: &Value) -> Value {
    let name = companion
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    json!({
        "type": "companion_intro",
        "name": name,
        "text": companion_intro_text(name, companion.get("species").and_then(|v| v.as_str()).unwrap_or("companion")),
    })
}

/// `getCompanionIntroAttachment`: gate on the feature switch, skip muted
/// companions, and dedupe against already-delivered intro attachments.
pub fn get_companion_intro_attachment(
    messages: Option<&[Value]>,
    companion: Option<&Value>,
) -> Vec<Value> {
    if !BUDDY_FEATURE_ENABLED.load(Ordering::Relaxed) {
        return vec![];
    }
    let Some(companion) = companion else {
        return vec![];
    };
    let muted = companion
        .get("muted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if muted {
        return vec![];
    }
    let payload = companion_intro_attachment(companion);
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let already = messages.unwrap_or(&[]).iter().any(|m| {
        m.get("attachment")
            .and_then(|a| a.get("type"))
            .and_then(|v| v.as_str())
            == Some("companion_intro")
            && m.get("attachment")
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                == Some(name)
    });
    if already {
        return vec![];
    }
    vec![payload]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn intro_text_names_and_species() {
        let t = companion_intro_text("Pixel", "cat");
        assert!(t.contains("A small cat named Pixel"));
        assert!(t.contains("You're not Pixel"));
    }

    #[test]
    fn gate_mute_and_dedup_sequential() {
        // Single sequential test: BUDDY_FEATURE_ENABLED is process-global,
        // so parallel tests would race on the switch.
        let c = json!({"name": "Pixel", "species": "cat"});
        BUDDY_FEATURE_ENABLED.store(true, Ordering::SeqCst);
        assert_eq!(get_companion_intro_attachment(None, Some(&c)).len(), 1);
        let muted = json!({"name": "Pixel", "species": "cat", "muted": true});
        assert!(get_companion_intro_attachment(None, Some(&muted)).is_empty());
        BUDDY_FEATURE_ENABLED.store(false, Ordering::Relaxed);
        assert!(get_companion_intro_attachment(None, Some(&c)).is_empty());
        BUDDY_FEATURE_ENABLED.store(true, Ordering::Relaxed);

        let history = vec![json!({
            "type": "attachment",
            "attachment": {"type": "companion_intro", "name": "Pixel"}
        })];
        assert!(get_companion_intro_attachment(Some(&history), Some(&c)).is_empty());
        let other = vec![json!({
            "type": "attachment",
            "attachment": {"type": "companion_intro", "name": "Other"}
        })];
        let out = get_companion_intro_attachment(Some(&other), Some(&c));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["name"], "Pixel");
    }
}

/// Registry-view wrapper preserving the original export name.
pub fn companionIntroText(args: &[Value]) -> Value {
    let name = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let species = args.get(1).and_then(|v| v.as_str()).unwrap_or("companion");
    Value::String(companion_intro_text(name, species))
}
