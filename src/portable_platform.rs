//! Portable Windows/WSL, POSIX shell, cron and version helpers.
#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Datelike, Local};
use regex::Regex;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

// src/utils/idePathConversion.ts
#[derive(Clone, Debug)]
pub struct WindowsToWslConverter {
    pub distro: Option<String>,
}
pub fn wsl_distro_matches(path: &str, distro: &str) -> bool {
    let re = Regex::new(r"(?i)^\\\\wsl(?:\.localhost|\$)\\([^\\]+)").unwrap();
    re.captures(path)
        .and_then(|c| c.get(1))
        .is_none_or(|m| m.as_str() == distro)
}
impl WindowsToWslConverter {
    pub fn to_local_path(&self, path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }
        if self
            .distro
            .as_deref()
            .is_some_and(|d| !wsl_distro_matches(path, d))
        {
            return path.into();
        }
        if let Ok(result) = std::process::Command::new("wslpath")
            .args(["-u", path])
            .output()
        {
            if result.status.success() {
                let s = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !s.is_empty() {
                    return s;
                }
            }
        }
        let slashes = path.replace('\\', "/");
        let b = slashes.as_bytes();
        if b.len() > 1 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            format!(
                "/mnt/{}{}",
                (b[0] as char).to_ascii_lowercase(),
                &slashes[2..]
            )
        } else {
            slashes
        }
    }
    pub fn to_ide_path(&self, path: &str) -> String {
        if let Ok(result) = std::process::Command::new("wslpath")
            .args(["-w", path])
            .output()
        {
            if result.status.success() {
                let text = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
        path.into()
    }
}

// src/utils/windowsPaths.ts
pub fn find_git_bash_path(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = override_path {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    for location in [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ] {
        let path = PathBuf::from(location);
        if path.is_file() {
            return Some(path);
        }
    }
    crate::shell_runtime::which("bash")
}
pub fn safe_windows_executable(candidate: &Path, cwd: &Path) -> bool {
    let path = candidate
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy()
        .to_lowercase();
    let cwd = cwd.to_string_lossy().to_lowercase();
    path != cwd && !path.starts_with(&(cwd + "\\"))
}

// src/utils/bash/shellPrefix.ts
pub fn quote_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
pub fn format_shell_prefix_command(prefix: &str, command: &str) -> String {
    if let Some(index) = prefix.rfind(" -").filter(|i| *i > 0) {
        format!(
            "{} {} {}",
            quote_argument(&prefix[..index]),
            &prefix[index + 1..],
            quote_argument(command)
        )
    } else {
        format!("{} {}", quote_argument(prefix), quote_argument(command))
    }
}

// src/utils/bash/shellQuoting.ts
pub fn contains_heredoc(command: &str) -> bool {
    if Regex::new(r"\d\s*<<\s*\d|\[\[\s*\d+\s*<<\s*\d+\s*\]\]|\$\(\(.*<<.*\)\)")
        .unwrap()
        .is_match(command)
    {
        return false;
    }
    Regex::new(r#"(?m)<<-?\s*(?:['"]?\w+['"]?|\\\w+)"#)
        .unwrap()
        .is_match(command)
}
pub fn contains_multiline_string(command: &str) -> bool {
    Regex::new(r#"(?s)'(?:[^'\\]|\\.)*\n(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*\n(?:[^"\\]|\\.)*""#)
        .unwrap()
        .is_match(command)
}
pub fn has_stdin_redirect(command: &str) -> bool {
    let c = command.chars().collect::<Vec<_>>();
    for i in 0..c.len() {
        if c[i] != '<' || (i > 0 && !matches!(c[i - 1], ' ' | '\t' | '\n' | ';' | '&' | '|')) {
            continue;
        }
        if matches!(c.get(i + 1), Some(&'<') | Some(&'(')) {
            continue;
        }
        if c[i + 1..]
            .iter()
            .skip_while(|ch| ch.is_whitespace())
            .next()
            .is_some()
        {
            return true;
        }
    }
    false
}
pub fn should_add_stdin_redirect(command: &str) -> bool {
    !contains_heredoc(command) && !has_stdin_redirect(command)
}
pub fn quote_shell_command(command: &str, redirect: bool) -> String {
    let value = quote_argument(command);
    if contains_heredoc(command) {
        value
    } else if redirect {
        format!("{value} < /dev/null")
    } else {
        value
    }
}
pub fn rewrite_windows_null_redirect(command: &str) -> String {
    let re = Regex::new(r"(?i)(\d?&?>+\s*)nul(?P<after>\s|$|[|&;)\n])").unwrap();
    re.replace_all(command, "$1/dev/null$after").into_owned()
}

// src/utils/cronJitterConfig.ts
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CronJitterConfig {
    pub recurring_frac: f64,
    pub recurring_cap_ms: u64,
    pub one_shot_max_ms: u64,
    pub one_shot_floor_ms: u64,
    pub one_shot_minute_mod: u64,
    pub recurring_max_age_ms: u64,
}
impl Default for CronJitterConfig {
    fn default() -> Self {
        Self {
            recurring_frac: 0.1,
            recurring_cap_ms: 900_000,
            one_shot_max_ms: 90_000,
            one_shot_floor_ms: 0,
            one_shot_minute_mod: 30,
            recurring_max_age_ms: 604_800_000,
        }
    }
}
pub fn parse_cron_jitter_config(value: &Value) -> CronJitterConfig {
    let default = CronJitterConfig::default();
    let read =
        |name: &str, max: u64| -> Option<u64> { value.get(name)?.as_u64().filter(|n| *n <= max) };
    (|| -> Option<CronJitterConfig> {
        let frac = value.get("recurringFrac")?.as_f64()?;
        if !(0.0..=1.0).contains(&frac) {
            return None;
        }
        let cap = read("recurringCapMs", 1_800_000)?;
        let max = read("oneShotMaxMs", 1_800_000)?;
        let floor = read("oneShotFloorMs", 1_800_000)?;
        let mod_min = read("oneShotMinuteMod", 60)?.max(1);
        if floor > max {
            return None;
        }
        let age = if value.get("recurringMaxAgeMs").is_some() {
            read("recurringMaxAgeMs", 2_592_000_000)?
        } else {
            default.recurring_max_age_ms
        };
        Some(CronJitterConfig {
            recurring_frac: frac,
            recurring_cap_ms: cap,
            one_shot_max_ms: max,
            one_shot_floor_ms: floor,
            one_shot_minute_mod: mod_min,
            recurring_max_age_ms: age,
        })
    })()
    .unwrap_or(default)
}

// src/utils/formatBriefTimestamp.ts
pub fn posix_locale(vars: &HashMap<String, String>) -> Option<String> {
    let value = ["LC_ALL", "LC_TIME", "LANG"]
        .iter()
        .find_map(|k| vars.get(*k).filter(|s| !s.is_empty()))?;
    if matches!(value.as_str(), "C" | "POSIX") {
        return None;
    }
    let value = value
        .split('.')
        .next()?
        .split('@')
        .next()?
        .replace('_', "-");
    (!value.is_empty()).then_some(value)
}
pub fn format_brief_timestamp(iso: &str, now: DateTime<Local>, locale: Option<&str>) -> String {
    let Ok(when) = DateTime::parse_from_rfc3339(iso) else {
        return String::new();
    };
    let local = when.with_timezone(&Local);
    let age = (now.date_naive() - local.date_naive()).num_days();
    let use_24h = locale.is_some_and(|s| {
        s.ends_with("-GB")
            || s.ends_with("-DE")
            || s.ends_with("-FR")
            || s.ends_with("-CN")
            || s.starts_with("zh")
    });
    let time = if use_24h {
        local.format("%H:%M").to_string()
    } else {
        local.format("%-I:%M %p").to_string()
    };
    if age == 0 {
        time
    } else if (1..7).contains(&age) {
        format!("{}, {time}", local.format("%A"))
    } else {
        format!(
            "{}, {} {}, {time}",
            local.format("%A"),
            local.format("%b"),
            local.day()
        )
    }
}

// src/utils/semver.ts, loose comparator with optional prerelease.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub stable: bool,
    pub prerelease: String,
}
pub fn parse_version(value: &str) -> Option<Version> {
    let text = value.trim().trim_start_matches('v').trim_start_matches('=');
    let (numbers, prerelease) = text
        .split_once('-')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let mut segments = numbers.split('+').next()?.split('.');
    Some(Version {
        major: segments.next()?.parse().ok()?,
        minor: segments.next().unwrap_or("0").parse().ok()?,
        patch: segments.next().unwrap_or("0").parse().ok()?,
        stable: prerelease.is_none(),
        prerelease: prerelease.unwrap_or("").into(),
    })
}
pub fn semver_order(a: &str, b: &str) -> Option<i8> {
    use std::cmp::Ordering;
    Some(match parse_version(a)?.cmp(&parse_version(b)?) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
}
pub fn semver_satisfies(value: &str, range: &str) -> bool {
    let parts = range.split_whitespace().collect::<Vec<_>>();
    !parts.is_empty()
        && parts.into_iter().all(|condition| {
            if condition == "*" {
                return true;
            }
            for (op, rule) in [(">=", 0i8), ("<=", -2), (">", 1), ("<", -1), ("=", 2)] {
                if let Some(reference) = condition.strip_prefix(op) {
                    return semver_order(value, reference).is_some_and(|order| match rule {
                        0 => order >= 0,
                        -2 => order <= 0,
                        1 => order > 0,
                        -1 => order < 0,
                        _ => order == 0,
                    });
                }
            }
            semver_order(value, condition) == Some(0)
        })
}

// src/utils/process.ts: swallow EPIPE for output streams.
pub fn write_stdout(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    match std::io::stdout().write_all(text.as_bytes()) {
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}
pub fn write_stderr(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    match std::io::stderr().write_all(text.as_bytes()) {
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bash_quote_and_paths() {
        let wsl = WindowsToWslConverter {
            distro: Some("Ubuntu".into()),
        };
        assert_eq!(wsl.to_local_path("C:\\Project\\a"), "/mnt/c/Project/a");
        assert!(!wsl_distro_matches(r"\\wsl$\Debian\home\a", "Ubuntu"));
        assert_eq!(
            format_shell_prefix_command("/usr/bin/bash -c", "echo ok"),
            "'/usr/bin/bash' -c 'echo ok'"
        );
        assert_eq!(
            rewrite_windows_null_redirect("cat a 2>NUL"),
            "cat a 2>/dev/null"
        );
        assert_eq!(
            rewrite_windows_null_redirect("echo >nul.txt"),
            "echo >nul.txt"
        );
        assert!(should_add_stdin_redirect("echo hello"));
        assert!(!should_add_stdin_redirect("cat < file"));
    }
    #[test]
    fn cron_and_semver() {
        let config = parse_cron_jitter_config(&serde_json::json!({
            "recurringFrac":0.4,"recurringCapMs":9000,
            "oneShotMaxMs":3000,"oneShotFloorMs":100,"oneShotMinuteMod":10}));
        assert_eq!(config.one_shot_max_ms, 3000);
        assert_eq!(config.recurring_max_age_ms, 604_800_000);
        assert_eq!(semver_order("v1.2.3", "1.2.4"), Some(-1));
        assert!(semver_satisfies("1.2.3", ">=1.0.0 <2.0.0"));
    }
}
