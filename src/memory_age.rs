//! Time-relative memory freshness. This module is intentionally pure so
//! fixtures can compare outputs with the original numeric/date contract.
use std::time::{SystemTime, UNIX_EPOCH};

pub const DAY_MS: f64 = 86_400_000.0;

pub fn now_millis() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(delta) => delta.as_secs_f64() * 1000.0,
        Err(delta) => -(delta.duration().as_secs_f64() * 1000.0),
    }
}

pub fn age_days_at(mtime_ms: f64, now_ms: f64) -> f64 {
    let days = ((now_ms - mtime_ms) / DAY_MS).floor();
    // JavaScript Math.max(0, NaN) returns NaN, unlike Rust f64::max.
    if days.is_nan() {
        f64::NAN
    } else {
        days.max(0.0)
    }
}

pub fn age_days(mtime_ms: f64) -> f64 {
    age_days_at(mtime_ms, now_millis())
}

fn days_label(days: f64) -> String {
    if days.is_nan() {
        "NaN".into()
    } else if days == f64::INFINITY {
        "Infinity".into()
    } else if days == f64::NEG_INFINITY {
        "-Infinity".into()
    } else {
        format!("{days}")
    }
}

pub fn age_at(mtime_ms: f64, now_ms: f64) -> String {
    let days = age_days_at(mtime_ms, now_ms);
    if days == 0.0 {
        "today".into()
    } else if days == 1.0 {
        "yesterday".into()
    } else {
        format!("{} days ago", days_label(days))
    }
}

pub fn age(mtime_ms: f64) -> String {
    age_at(mtime_ms, now_millis())
}

pub fn freshness_text_at(mtime_ms: f64, now_ms: f64) -> String {
    let days = age_days_at(mtime_ms, now_ms);
    if days <= 1.0 {
        return String::new();
    }
    format!(
        "This memory is {} days old. Memories are point-in-time observations, not live state — claims about code behavior or file:line citations may be outdated. Verify against current code before asserting as fact.",
        days_label(days)
    )
}

pub fn freshness_text(mtime_ms: f64) -> String {
    freshness_text_at(mtime_ms, now_millis())
}

pub fn freshness_note_at(mtime_ms: f64, now_ms: f64) -> String {
    let text = freshness_text_at(mtime_ms, now_ms);
    if text.is_empty() {
        String::new()
    } else {
        format!("<system-reminder>{text}</system-reminder>\n")
    }
}

pub fn freshness_note(mtime_ms: f64) -> String {
    freshness_note_at(mtime_ms, now_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_age_semantics_and_exact_messages() {
        let now = 1_800_000_000_000.0;
        for (elapsed, expected_days, expected_age) in [
            (-5_000.0, 0.0, "today"),
            (0.0, 0.0, "today"),
            (86_399_999.0, 0.0, "today"),
            (86_400_000.0, 1.0, "yesterday"),
            (172_799_999.0, 1.0, "yesterday"),
            (172_800_000.0, 2.0, "2 days ago"),
            (4_060_800_000.0, 47.0, "47 days ago"),
        ] {
            let modified = now - elapsed;
            assert_eq!(age_days_at(modified, now), expected_days);
            assert_eq!(age_at(modified, now), expected_age);
        }
        assert!(freshness_text_at(now - DAY_MS, now).is_empty());
        assert!(freshness_note_at(now, now).is_empty());
        let note = freshness_note_at(now - 2.0 * DAY_MS, now);
        assert_eq!(note,
            "<system-reminder>This memory is 2 days old. Memories are point-in-time observations, not live state — claims about code behavior or file:line citations may be outdated. Verify against current code before asserting as fact.</system-reminder>\n");
    }
    #[test]
    fn special_numeric_inputs_match_number_semantics() {
        assert!(age_days_at(f64::NAN, 0.0).is_nan());
        assert_eq!(age_at(f64::NAN, 0.0), "NaN days ago");
        assert_eq!(age_at(f64::NEG_INFINITY, 0.0), "Infinity days ago");
        assert_eq!(age_at(f64::INFINITY, 0.0), "today");
        assert!(freshness_text_at(f64::NAN, 0.0).contains("NaN days old"));
    }
}
