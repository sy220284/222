//! Rust exports derived from original: src/buddy/prompt.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn companionIntroText(args: &[Value]) -> Value {
    super::runtime::concat(&[Value::String("# Companion\n\nA small ".into()),args.get(1).cloned().unwrap_or(Value::Null),Value::String(" named ".into()),args.first().cloned().unwrap_or(Value::Null),Value::String(" sits beside the user's input box and occasionally comments in a speech bubble. You're not ".into()),args.first().cloned().unwrap_or(Value::Null),Value::String(" — it's a separate watcher.\n\nWhen the user addresses ".into()),args.first().cloned().unwrap_or(Value::Null),Value::String(" directly (by name), its bubble will answer. Your job in that moment is to stay out of the way: respond in ONE line or less, or just answer any part of the message meant for you. Don't explain that you're not ".into()),args.first().cloned().unwrap_or(Value::Null),Value::String(" — they know. Don't narrate what ".into()),args.first().cloned().unwrap_or(Value::Null),Value::String(" might say — the bubble handles that.".into())])
}
