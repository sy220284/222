//! Native Rust port derived from original: src/utils/diagLogs.ts
//!
//! Diagnostic logging for container session-ingress monitoring. JSON-lines
//! appended to `ATLAS_DIAGNOSTICS_FILE`; directory creation on first append
//! failure; timing wrapper logs `{event}_started/_completed/_failed` with
//! `duration_ms`. **Must never receive PII** — same contract as original.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLogLevel {
    debug,
    info,
    warn,
    error,
}

impl DiagnosticLogLevel {
    fn as_str(self) -> &'static str {
        match self {
            DiagnosticLogLevel::debug => "debug",
            DiagnosticLogLevel::info => "info",
            DiagnosticLogLevel::warn => "warn",
            DiagnosticLogLevel::error => "error",
        }
    }
}

/// One JSON-lines diagnostic entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticLogEntry {
    pub timestamp: String,
    pub level: DiagnosticLogLevel,
    pub event: String,
    #[serde(default)]
    pub data: Map<String, Value>,
}

/// Diagnostic log target from the environment; `None` disables logging.
pub fn get_diagnostic_log_file() -> Option<PathBuf> {
    match std::env::var("ATLAS_DIAGNOSTICS_FILE") {
        Ok(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => None,
    }
}

fn append_line(log_file: &Path, line: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;
    file.write_all(line.as_bytes())
}

/// Logs diagnostic information without PII (sync IO, called from sync
/// context — mirrors the original's contract).
pub fn log_for_diagnostics_no_pii(
    level: DiagnosticLogLevel,
    event: &str,
    data: Option<Map<String, Value>>,
) {
    let Some(log_file) = get_diagnostic_log_file() else {
        return;
    };
    let entry = DiagnosticLogEntry {
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        level,
        event: event.to_string(),
        data: data.unwrap_or_default(),
    };
    let line = serde_json::to_string(&entry).unwrap_or_else(|_| "{}".to_string()) + "\n";
    if append_line(&log_file, &line).is_ok() {
        return;
    }
    // If append fails, try creating the directory first.
    if let Some(dir) = log_file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = append_line(&log_file, &line); // Silently fail if logging is not possible.
}

/// Convenience wrapper accepting a serde-built data object.
pub fn log_diag(level: DiagnosticLogLevel, event: &str, data: Value) {
    let map = match data {
        Value::Object(map) => map,
        Value::Null => Map::new(),
        other => {
            let mut map = Map::new();
            map.insert("value".to_string(), other);
            map
        }
    };
    log_for_diagnostics_no_pii(level, event, Some(map));
}

/// Wraps an async closure with diagnostic timing logs: `{event}_started`
/// before execution, `{event}_completed` (with `duration_ms` plus optional
/// extracted data) on success, `{event}_failed` on error — then re-raises.
pub async fn with_diagnostics_timing<T, E, F, Fut, D>(
    event: &str,
    f: F,
    get_data: D,
) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    D: FnOnce(&T) -> Map<String, Value>,
{
    let start_time = std::time::Instant::now();
    log_for_diagnostics_no_pii(DiagnosticLogLevel::info, &format!("{event}_started"), None);
    match f().await {
        Ok(result) => {
            let mut data = get_data(&result);
            data.insert(
                "duration_ms".to_string(),
                json!(u128_to_ms(start_time.elapsed())),
            );
            log_for_diagnostics_no_pii(
                DiagnosticLogLevel::info,
                &format!("{event}_completed"),
                Some(data),
            );
            Ok(result)
        }
        Err(error) => {
            log_for_diagnostics_no_pii(
                DiagnosticLogLevel::error,
                &format!("{event}_failed"),
                Some(
                    json!({ "duration_ms": u128_to_ms(start_time.elapsed()) })
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                ),
            );
            Err(error)
        }
    }
}

fn u128_to_ms(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

use std::future::Future;

#[cfg(test)]
mod tests {
    use super::*#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    const DIAGNOSTIC_ENV: &str = "ATLAS_DIAGNOSTICS_FILE";

    // The variable belongs to the whole process, so tests must serialize
    // changes and restore the caller's environment even on assertion panic.
    struct DiagnosticEnvGuard {
        _lock: MutexGuard<'static, ()>,
        previous: Option<OsString>,
    }

    impl DiagnosticEnvGuard {
        fn set(path: Option<&Path>) -> Self {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let lock = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
            let previous = std::env::var_os(DIAGNOSTIC_ENV);
            if let Some(path) = path {
                std::env::set_var(DIAGNOSTIC_ENV, path);
            } else {
                std::env::remove_var(DIAGNOSTIC_ENV);
            }
            Self {
                _lock: lock,
                previous,
            }
        }
    }

    impl Drop for DiagnosticEnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(DIAGNOSTIC_ENV, previous);
            } else {
                std::env::remove_var(DIAGNOSTIC_ENV);
            }
        }
    }

    #[test]
    fn writes_json_line_to_env_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("diag.log");
        let _env = DiagnosticEnvGuard::set(Some(&path));
        log_for_diagnostics_no_pii(DiagnosticLogLevel::info, "mcp_connected", None);
        let content = fs::read_to_string(&path).expect("nested dir must be auto-created");
        let entry: DiagnosticLogEntry =
            serde_json::from_str(content.lines().last().unwrap()).unwrap();
        assert_eq!(entry.event, "mcp_connected");
        assert_eq!(entry.level, DiagnosticLogLevel::info);
        assert!(entry.timestamp.contains('T'));
    }

    #[test]
    fn disabled_without_env() {
        let _env = DiagnosticEnvGuard::set(None);
        // Must be a no-op, not a panic.
        log_for_diagnostics_no_pii(DiagnosticLogLevel::error, "should_not_write", None);
    }

    #[tokio::test]
    async fn timing_wrapper_logs_started_completed_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diag.log");
        let _env = DiagnosticEnvGuard::set(Some(&path));

        let ok: Result<u32, ()> = with_diagnostics_timing(
            "git_status",
            || async { Ok(42) },
            |r| json!({ "value": r }).as_object().cloned().unwrap(),
        )
        .await;
        assert_eq!(ok.unwrap(), 42);

        let err: Result<u32, &str> =
            with_diagnostics_timing("boom", || async { Err("x") }, |_| Map::new()).await;
        assert!(err.is_err());

        let content = fs::read_to_string(&path).unwrap();
        let events: Vec<String> = content
            .lines()
            .filter_map(|l| serde_json::from_str::<DiagnosticLogEntry>(l).ok())
            .map(|e| e.event)
            .collect();
        assert!(events.iter().any(|event| event == "git_status_started"));
        assert!(events.iter().any(|event| event == "git_status_completed"));
        assert!(events.iter().any(|event| event == "boom_started"));
        assert!(events.iter().any(|event| event == "boom_failed"));
    }
}
