//! Leaf display formatting from the original format utility.
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};

pub use crate::generated_ports::terminal_truncate::{
    truncate, truncate_path_middle, truncate_start_to_width, truncate_to_width,
    truncate_to_width_no_ellipsis, wrap_text,
};

fn number(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else if value == 0.0 {
        "0".into()
    } else if value.abs() >= 1e21 || value.abs() < 1e-6 {
        let raw = value.abs().to_string();
        let decimal = raw.find('.').unwrap_or(raw.len());
        let digits = raw.replace('.', "");
        let first = digits.find(|c: char| c != '0').unwrap_or(0);
        let significant = digits[first..].trim_end_matches('0');
        let exponent = decimal as i32 - first as i32 - 1;
        let mantissa = if significant.len() > 1 {
            format!("{}.{}", &significant[..1], &significant[1..])
        } else {
            significant.into()
        };
        format!(
            "{}{mantissa}e{}{exponent}",
            if value < 0.0 { "-" } else { "" },
            if exponent >= 0 { "+" } else { "" }
        )
    } else {
        value.to_string()
    }
}
fn fixed_one(value: f64, trim: bool) -> String {
    if !value.is_finite() || value.abs() >= 1e21 {
        return number(value);
    }
    // Round the actual binary value, as Number.toFixed does. Multiplying
    // by 10 in f64 first would incorrectly turn 1.15 into an exact 11.5.
    let bits = value.abs().to_bits();
    let encoded = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = (bits & ((1u64 << 52) - 1)) | if encoded == 0 { 0 } else { 1u64 << 52 };
    let exponent = if encoded == 0 {
        -1074
    } else {
        encoded - 1023 - 52
    } + 1;
    let numerator = mantissa as u128 * 5;
    let rounded = if exponent >= 0 {
        numerator.checked_shl(exponent as u32)
    } else {
        let shift = (-exponent) as u32;
        Some(if shift >= 128 {
            0
        } else {
            (numerator + (1u128 << (shift - 1))) >> shift
        })
    };
    let value = match rounded {
        Some(n) => format!(
            "{}{}.{}",
            if value < 0.0 { "-" } else { "" },
            n / 10,
            n % 10
        ),
        None => number(value),
    };
    if trim {
        value.strip_suffix(".0").unwrap_or(&value).to_owned()
    } else {
        value
    }
}
fn compact_one(value: f64, trim: bool) -> String {
    let value = format!("{:.1}", (value * 10.0).round() / 10.0);
    if trim {
        value.strip_suffix(".0").unwrap_or(&value).to_owned()
    } else {
        value
    }
}
pub fn format_file_size(bytes: f64) -> String {
    if bytes < 1024.0 {
        return format!("{} bytes", number(bytes));
    }
    for (scale, suffix) in [(1024.0, "KB"), (1048576.0, "MB")] {
        if bytes / scale < 1024.0 {
            return format!("{}{suffix}", fixed_one(bytes / scale, true));
        }
    }
    format!("{}GB", fixed_one(bytes / 1073741824.0, true))
}
pub fn format_seconds_short(ms: f64) -> String {
    format!("{}s", fixed_one(ms / 1000.0, false))
}
#[derive(Clone, Copy, Default)]
pub struct DurationOptions {
    pub hide_trailing_zeros: bool,
    pub most_significant_only: bool,
}
pub fn format_duration(ms: f64, options: DurationOptions) -> String {
    if ms < 60000.0 {
        return if ms == 0.0 {
            "0s".into()
        } else if ms < 1.0 {
            format_seconds_short(ms)
        } else {
            format!("{}s", number((ms / 1000.0).floor()))
        };
    }
    let mut days = (ms / 86400000.0).floor();
    let mut hours = ((ms % 86400000.0) / 3600000.0).floor();
    let mut minutes = ((ms % 3600000.0) / 60000.0).floor();
    let mut seconds = ((ms % 60000.0) / 1000.0).round();
    if seconds == 60.0 {
        seconds = 0.0;
        minutes += 1.0;
    }
    if minutes == 60.0 {
        minutes = 0.0;
        hours += 1.0;
    }
    if hours == 24.0 {
        hours = 0.0;
        days += 1.0;
    }
    if options.most_significant_only {
        for (value, suffix) in [(days, "d"), (hours, "h"), (minutes, "m")] {
            if value > 0.0 {
                return format!("{}{suffix}", number(value));
            }
        }
        return format!("{}s", number(seconds));
    }
    let hide = options.hide_trailing_zeros;
    if days > 0.0 {
        if hide && hours == 0.0 && minutes == 0.0 {
            return format!("{}d", number(days));
        }
        if hide && minutes == 0.0 {
            return format!("{}d {}h", number(days), number(hours));
        }
        return format!("{}d {}h {}m", number(days), number(hours), number(minutes));
    }
    if hours > 0.0 {
        if hide && minutes == 0.0 && seconds == 0.0 {
            return format!("{}h", number(hours));
        }
        if hide && seconds == 0.0 {
            return format!("{}h {}m", number(hours), number(minutes));
        }
        return format!(
            "{}h {}m {}s",
            number(hours),
            number(minutes),
            number(seconds)
        );
    }
    if minutes > 0.0 {
        if hide && seconds == 0.0 {
            return format!("{}m", number(minutes));
        }
        return format!("{}m {}s", number(minutes), number(seconds));
    }
    format!("{}s", number(seconds))
}
pub fn format_number(value: f64) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-∞".into()
        } else {
            "∞".into()
        };
    }
    let consistent = value >= 1000.0;
    let mut scale = 1.0;
    let mut suffix = "";
    for (candidate, unit) in [(1e3, "k"), (1e6, "m"), (1e9, "b"), (1e12, "t")] {
        if (value.abs() / scale * 10.0).round() / 10.0 >= candidate / scale {
            scale = candidate;
            suffix = unit;
        }
    }
    format!("{}{suffix}", compact_one(value / scale, !consistent))
}
pub fn format_tokens(value: f64) -> String {
    format_number(value).replacen(".0", "", 1)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RelativeStyle {
    Long,
    Short,
    #[default]
    Narrow,
}
#[derive(Clone, Copy, Default)]
pub struct RelativeTimeOptions {
    pub style: RelativeStyle,
    pub numeric_auto: bool,
}
pub fn format_relative_time(date_ms: i64, now_ms: i64, options: RelativeTimeOptions) -> String {
    let seconds = (date_ms as i128 - now_ms as i128) / 1000;
    for (unit, interval, short) in [
        ("year", 31536000, "y"),
        ("month", 2592000, "mo"),
        ("week", 604800, "w"),
        ("day", 86400, "d"),
        ("hour", 3600, "h"),
        ("minute", 60, "m"),
        ("second", 1, "s"),
    ] {
        if seconds.abs() < interval {
            continue;
        }
        let value = seconds / interval;
        if options.style == RelativeStyle::Narrow {
            return if seconds < 0 {
                format!("{}{short} ago", value.abs())
            } else {
                format!("in {value}{short}")
            };
        }
        if options.numeric_auto {
            if matches!(unit, "year" | "month" | "week" | "day") && value.abs() <= 1 {
                return match (unit, value) {
                    ("day", -1) => "yesterday".into(),
                    ("day", 1) => "tomorrow".into(),
                    (_, -1) => format!("last {unit}"),
                    (_, 1) => format!("next {unit}"),
                    _ => format!("this {unit}"),
                };
            }
        }
        let count = value.abs();
        let plural = if count == 1 { "" } else { "s" };
        return if value < 0 {
            format!("{count} {unit}{plural} ago")
        } else {
            format!("in {count} {unit}{plural}")
        };
    }
    if options.style == RelativeStyle::Narrow {
        "0s ago".into()
    } else if options.numeric_auto {
        "now".into()
    } else if options.style == RelativeStyle::Short {
        "in 0 sec.".into()
    } else {
        "in 0 seconds".into()
    }
}
pub fn format_relative_time_ago(
    date_ms: i64,
    now_ms: i64,
    mut options: RelativeTimeOptions,
) -> String {
    if date_ms <= now_ms {
        options.numeric_auto = false;
    }
    format_relative_time(date_ms, now_ms, options)
}
pub struct LogMetadata<'a> {
    pub modified_ms: i64,
    pub message_count: usize,
    pub file_size: Option<u64>,
    pub git_branch: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub agent_setting: Option<&'a str>,
    pub pr_number: Option<u64>,
    pub pr_repository: Option<&'a str>,
}
pub fn format_log_metadata(log: &LogMetadata<'_>, now_ms: i64) -> String {
    let mut parts = vec![format_relative_time_ago(
        log.modified_ms,
        now_ms,
        RelativeTimeOptions {
            style: RelativeStyle::Short,
            numeric_auto: false,
        },
    )];
    if let Some(branch) = log.git_branch.filter(|v| !v.is_empty()) {
        parts.push(branch.into());
    }
    parts.push(
        log.file_size
            .map(|n| format_file_size(n as f64))
            .unwrap_or_else(|| format!("{} messages", log.message_count)),
    );
    if let Some(tag) = log.tag.filter(|v| !v.is_empty()) {
        parts.push(format!("#{tag}"));
    }
    if let Some(agent) = log.agent_setting.filter(|v| !v.is_empty()) {
        parts.push(format!("@{agent}"));
    }
    if let Some(number) = log.pr_number.filter(|n| *n != 0) {
        parts.push(match log.pr_repository.filter(|v| !v.is_empty()) {
            Some(repo) => format!("{repo}#{number}"),
            None => format!("#{number}"),
        });
    }
    parts.join(" · ")
}
fn clock(date: &DateTime<FixedOffset>) -> String {
    let (pm, hour) = date.hour12();
    let time = if date.minute() == 0 {
        hour.to_string()
    } else {
        format!("{hour}:{:02}", date.minute())
    };
    format!("{time}{}", if pm { "pm" } else { "am" })
}
pub fn format_reset_time(
    timestamp: Option<i64>,
    show_timezone: bool,
    show_time: bool,
    now: DateTime<FixedOffset>,
    zone: &str,
) -> Result<Option<String>> {
    let Some(timestamp) = timestamp.filter(|v| *v != 0) else {
        return Ok(None);
    };
    let date = DateTime::from_timestamp(timestamp, 0)
        .context("Invalid reset timestamp")?
        .with_timezone(now.offset());
    let mut result = if date.signed_duration_since(now).num_milliseconds() as f64 / 3600000.0 > 24.0
    {
        let mut s = date.format("%b %-d").to_string();
        if date.year() != now.year() {
            s.push_str(&format!(", {}", date.year()));
        }
        if show_time {
            s.push_str(&format!(", {}", clock(&date)));
        }
        s
    } else {
        clock(&date)
    };
    if show_timezone {
        result.push_str(&format!(" ({zone})"));
    }
    Ok(Some(result))
}
pub fn format_reset_text(
    text: &str,
    show_timezone: bool,
    show_time: bool,
    now: DateTime<FixedOffset>,
    zone: &str,
) -> Result<String> {
    let date = DateTime::parse_from_rfc3339(text)
        .map(|d| d.timestamp())
        .or_else(|_| {
            NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
        })
        .context("Invalid reset date")?;
    Ok(
        format_reset_time(Some(date), show_timezone, show_time, now, zone)?
            .unwrap_or_else(|| "undefined".into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_formatter_snapshots() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/display_format_original.json"
        ))
        .unwrap();
        for (key, formatter) in [
            ("sizes", format_file_size as fn(f64) -> String),
            ("seconds", format_seconds_short),
            ("numbers", format_number),
            ("tokens", format_tokens),
        ] {
            for case in fixture[key].as_array().unwrap() {
                let n = case[0].as_f64().unwrap();
                assert_eq!(formatter(n), case[1].as_str().unwrap(), "{key}({n})");
            }
        }
        for case in fixture["durations"].as_array().unwrap() {
            let options = DurationOptions {
                hide_trailing_zeros: case[1]["hideTrailingZeros"] == true,
                most_significant_only: case[1]["mostSignificantOnly"] == true,
            };
            assert_eq!(
                format_duration(case[0].as_f64().unwrap(), options),
                case[2].as_str().unwrap(),
                "{case}"
            );
        }
    }
    #[test]
    fn file_sizes_and_duration_carries_match_original() {
        for (bytes, expected) in [
            (0.0, "0 bytes"),
            (1023.0, "1023 bytes"),
            (1024.0, "1KB"),
            (1536.0, "1.5KB"),
            (1048576.0, "1MB"),
        ] {
            assert_eq!(format_file_size(bytes), expected);
        }
        let normal = DurationOptions::default();
        for (ms, expected) in [
            (0.0, "0s"),
            (500.0, "0s"),
            (59999.0, "59s"),
            (60000.0, "1m 0s"),
            (3599500.0, "1h 0m 0s"),
            (86400000.0, "1d 0h 0m"),
        ] {
            assert_eq!(format_duration(ms, normal), expected);
        }
        assert_eq!(
            format_duration(
                3600000.0,
                DurationOptions {
                    hide_trailing_zeros: true,
                    most_significant_only: false
                }
            ),
            "1h"
        );
        assert_eq!(format_seconds_short(1234.0), "1.2s");
    }
    #[test]
    fn compact_numbers_keep_consistent_decimals_only_for_positive_thousands() {
        for (value, expected) in [
            (999.99, "1k"),
            (900.0, "900"),
            (1000.0, "1.0k"),
            (1321.0, "1.3k"),
            (1000000.0, "1.0m"),
            (-1000.0, "-1k"),
        ] {
            assert_eq!(format_number(value), expected);
        }
        assert_eq!(format_tokens(1000.0), "1k");
        assert_eq!(format_tokens(1321.0), "1.3k");
    }
    #[test]
    fn relative_times_truncate_and_force_numeric_for_past_logs() {
        let normal = RelativeTimeOptions::default();
        assert_eq!(format_relative_time(59999, 0, normal), "in 59s");
        assert_eq!(format_relative_time(0, 60000, normal), "1m ago");
        let auto = RelativeTimeOptions {
            style: RelativeStyle::Long,
            numeric_auto: true,
        };
        assert_eq!(format_relative_time(0, 86400000, auto), "yesterday");
        assert_eq!(format_relative_time_ago(0, 86400000, auto), "1 day ago");
        assert_eq!(format_relative_time(0, 0, auto), "now");
        assert_eq!(format_relative_time(172800000, 0, auto), "in 2 days");
    }
}
