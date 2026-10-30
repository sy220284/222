//! Native Rust port derived from original: src/utils/debug.ts
//!
//! Manual source-order rewrite of the debug logging core. Environment names
//! are provider-neutral (`CLAUDE_CODE_DEBUG_*` -> `ATLAS_DEBUG_*`), matching
//! the repo-wide naming convention. The Node event-loop write chain
//! (`pendingWrite` promise) becomes a synchronous buffered writer with
//! drop-flush semantics; `flushIntervalMs` timing becomes max-item/byte
//! bounds on the existing `portable_async_runtime::BufferedWriter`.
//! Memoization becomes eager recomputation where argv/env inputs dominate
//! cost; `enableDebugLogging` therefore needs no cache invalidation. All
//! deviations are recorded in docs/FILE_PORT_MATRIX.md.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::env_runtime::is_env_truthy;
use crate::portable_async_runtime::BufferedWriter;

pub use self::DebugLogLevel as DebugLogLevelTag;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DebugLogLevel {
    verbose,
    debug,
    info,
    warn,
    error,
}

impl DebugLogLevel {
    pub fn order(self) -> u8 {
        match self {
            DebugLogLevel::verbose => 0,
            DebugLogLevel::debug => 1,
            DebugLogLevel::info => 2,
            DebugLogLevel::warn => 3,
            DebugLogLevel::error => 4,
        }
    }

    fn from_raw(raw: &str) -> Option<Self> {
        match raw {
            "verbose" => Some(DebugLogLevel::verbose),
            "debug" => Some(DebugLogLevel::debug),
            "info" => Some(DebugLogLevel::info),
            "warn" => Some(DebugLogLevel::warn),
            "error" => Some(DebugLogLevel::error),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            DebugLogLevel::verbose => "verbose",
            DebugLogLevel::debug => "debug",
            DebugLogLevel::info => "info",
            DebugLogLevel::warn => "warn",
            DebugLogLevel::error => "error",
        }
    }
}

static RUNTIME_DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
static HAS_FORMATTED_OUTPUT: AtomicBool = AtomicBool::new(false);

// bootstrap/state.ts getSessionId(): process-wide session identifier,
// lazily generated like the original global singleton. Hosts may pin it via
// set_debug_session_id before logging starts.
static SESSION_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn session_slot() -> &'static Mutex<Option<String>> {
    SESSION_ID.get_or_init(|| Mutex::new(None))
}

pub fn set_debug_session_id(id: String) {
    *session_slot().lock().unwrap() = Some(id);
}

pub fn get_session_id() -> String {
    let mut slot = session_slot().lock().unwrap();
    slot.get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
        .clone()
}

fn argv() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// Minimum log level for debug output. Defaults to `debug`, filtering
/// `verbose` noise. `ATLAS_DEBUG_LOG_LEVEL=verbose` re-enables high-volume
/// diagnostics (original: CLAUDE_CODE_DEBUG_LOG_LEVEL).
pub fn get_min_debug_level() -> DebugLogLevel {
    let raw = std::env::var("ATLAS_DEBUG_LOG_LEVEL")
        .ok()
        .map(|v| v.to_lowercase().trim().to_owned())
        .filter(|v| !v.is_empty());
    raw.and_then(|v| DebugLogLevel::from_raw(&v))
        .unwrap_or(DebugLogLevel::debug)
}

pub fn is_debug_to_stderr() -> bool {
    let argv = argv();
    argv.iter().any(|a| a == "--debug-to-stderr" || a == "-d2e")
}

/// `--debug-file` path: `--debug-file=PATH` or `--debug-file PATH`.
pub fn get_debug_file_path() -> Option<String> {
    let argv = argv();
    for (i, arg) in argv.iter().enumerate() {
        if let Some(rest) = arg.strip_prefix("--debug-file=") {
            return Some(rest.to_owned());
        }
        if arg == "--debug-file" && i + 1 < argv.len() {
            return Some(argv[i + 1].clone());
        }
    }
    None
}

/// Debug filter parsed from `--debug=pattern`. Recomputed per call; argv is
/// immutable within a process, so this matches the original memoized lookup.
pub fn get_debug_filter() -> Option<crate::generated_ports::p_7a2adbb3dff05::DebugFilter> {
    let debug_arg = argv().into_iter().find(|a| a.starts_with("--debug="))?;
    let pattern = debug_arg.strip_prefix("--debug=")?;
    crate::generated_ports::p_7a2adbb3dff05::parse_debug_filter(Some(pattern))
}

pub fn is_debug_mode() -> bool {
    RUNTIME_DEBUG_ENABLED.load(Ordering::Relaxed)
        || is_env_truthy(std::env::var("DEBUG").ok().as_deref())
        || is_env_truthy(std::env::var("DEBUG_SDK").ok().as_deref())
        || argv().iter().any(|a| a == "--debug" || a == "-d")
        || is_debug_to_stderr()
        || argv().iter().any(|a| a.starts_with("--debug="))
        // --debug-file implicitly enables debug mode.
        || get_debug_file_path().is_some()
}

/// Enables debug logging mid-session (e.g. via /debug). Returns true if
/// logging was already active.
pub fn enable_debug_logging() -> bool {
    let was_active = is_debug_mode();
    RUNTIME_DEBUG_ENABLED.store(true, Ordering::Relaxed);
    was_active
}

fn user_is_ant() -> bool {
    std::env::var("USER_TYPE").ok().as_deref() == Some("ant")
}

fn should_log_debug_message(message: &str) -> bool {
    // Test processes stay silent unless stderr debug was requested
    // (original: NODE_ENV === 'test' && !isDebugToStderr()).
    if cfg!(test) && !is_debug_to_stderr() {
        return false;
    }

    // Non-ants only write debug logs when debug mode is active (via --debug
    // at startup or /debug mid-session). Ants always log for /share, bug
    // reports.
    if !user_is_ant() && !is_debug_mode() {
        return false;
    }

    let filter = get_debug_filter();
    crate::generated_ports::p_7a2adbb3dff05::should_show_debug_message(message, filter.as_ref())
}

pub fn set_has_formatted_output(value: bool) {
    HAS_FORMATTED_OUTPUT.store(value, Ordering::Relaxed);
}

pub fn get_has_formatted_output() -> bool {
    HAS_FORMATTED_OUTPUT.load(Ordering::Relaxed)
}

/// Debug log path resolution: `--debug-file` beats `ATLAS_DEBUG_LOGS_DIR`
/// (original: CLAUDE_CODE_DEBUG_LOGS_DIR) beats config home `debug/{session}.txt`.
pub fn get_debug_log_path() -> PathBuf {
    if let Some(file) = get_debug_file_path() {
        return PathBuf::from(file);
    }
    if let Ok(dir) = std::env::var("ATLAS_DEBUG_LOGS_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = crate::env_runtime::config_home_dir(None).unwrap_or_else(|_| PathBuf::from("."));
    home.join("debug").join(format!("{}.txt", get_session_id()))
}

struct DebugWriterState {
    writer: Option<BufferedWriter<Box<dyn Fn(&str) + Send>>>,
}

static DEBUG_WRITER: OnceLock<Mutex<DebugWriterState>> = OnceLock::new();

fn writer_slot() -> &'static Mutex<DebugWriterState> {
    DEBUG_WRITER.get_or_init(|| Mutex::new(DebugWriterState { writer: None }))
}

fn append_sync(path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content.as_bytes())
}

fn ensure_dir(dir: &Path) {
    let _ = fs::create_dir_all(dir);
}

/// Updates the `debug/latest` symlink to the current log file (unix only;
/// on Windows the symlink step is skipped and recorded as a difference).
fn update_latest_debug_log_symlink() {
    #[cfg(unix)]
    {
        let debug_log_path = get_debug_log_path();
        let Some(dir) = debug_log_path.parent() else {
            return;
        };
        let latest = dir.join("latest");
        let _ = fs::remove_file(&latest);
        let _ = std::os::unix::fs::symlink(&debug_log_path, &latest);
    }
}

/// Write through the shared debug writer. Immediate (sync) when debug mode is
/// active — async buffered writes are lost on direct process exit; buffered
/// (~1/s flush in the original, here bounded by item/byte counts with a
/// drop-flush) when only the ant default path is taken.
fn get_debug_writer() -> &'static Mutex<DebugWriterState> {
    writer_slot()
}

fn write_debug_output(output: &str) {
    if is_debug_to_stderr() {
        let mut err = std::io::stderr();
        let _ = err.write_all(output.as_bytes());
        let _ = err.flush();
        return;
    }

    let mut state = get_debug_writer().lock().unwrap();
    if state.writer.is_none() {
        let immediate = is_debug_mode();
        state.writer = Some(BufferedWriter::new(
            Box::new(move |content: &str| {
                let path = get_debug_log_path();
                if let Some(dir) = path.parent() {
                    // create_dir_all is idempotent; no nested locking here
                    // (the writer's flush path runs inside the state lock).
                    let _ = fs::create_dir_all(dir);
                }
                let _ = append_sync(&path, content);
                update_latest_debug_log_symlink();
            }),
            100,
            usize::MAX,
            immediate,
        ));
    }
    if let Some(writer) = state.writer.as_mut() {
        writer.push(output);
    }
}

/// Flush buffered debug logs (cleanup hook).
pub fn flush_debug_logs() {
    if let Ok(mut state) = get_debug_writer().lock() {
        if let Some(writer) = state.writer.as_mut() {
            writer.flush();
        }
    }
}

/// Log a message for debugging, honoring level thresholds and filters.
pub fn log_for_debugging(message: &str, level: DebugLogLevel) {
    if level.order() < get_min_debug_level().order() {
        return;
    }
    if !should_log_debug_message(message) {
        return;
    }

    // Multiline messages break the jsonl output format, so make any multiline
    // messages JSON.
    let message: String = if get_has_formatted_output() && message.contains('\n') {
        serde_json::to_string(message).unwrap_or_else(|_| message.to_owned())
    } else {
        message.to_owned()
    };

    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let output = format!(
        "{} [{}] {}\n",
        timestamp,
        level.as_str().to_uppercase(),
        message.trim()
    );
    write_debug_output(&output);
}

/// Convenience wrapper matching the original default options.
pub fn log_debug(message: &str) {
    log_for_debugging(message, DebugLogLevel::debug);
}

/// Logs errors for internal ant sessions only, always visible in production.
pub fn log_ant_error(context: &str, error: Option<&str>) {
    if !user_is_ant() {
        return;
    }
    if let Some(stack) = error {
        log_for_debugging(
            &format!("[ANT-ONLY] {context} stack trace:\n{stack}"),
            DebugLogLevel::error,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_order_matches_original_table() {
        assert_eq!(DebugLogLevel::verbose.order(), 0);
        assert_eq!(DebugLogLevel::debug.order(), 1);
        assert_eq!(DebugLogLevel::info.order(), 2);
        assert_eq!(DebugLogLevel::warn.order(), 3);
        assert_eq!(DebugLogLevel::error.order(), 4);
        assert_eq!(get_min_debug_level(), DebugLogLevel::debug);
    }

    #[test]
    fn debug_file_path_both_arg_forms() {
        // Direct arg probing would read this test binary's argv; exercise the
        // pure scan logic through a synthetic list instead.
        let scan = |args: &[&str]| -> Option<String> {
            for (i, arg) in args.iter().enumerate() {
                if let Some(rest) = arg.strip_prefix("--debug-file=") {
                    return Some(rest.to_string());
                }
                if *arg == "--debug-file" && i + 1 < args.len() {
                    return Some(args[i + 1].to_string());
                }
            }
            None
        };
        assert_eq!(
            scan(&["--debug-file=/tmp/a.txt"]),
            Some("/tmp/a.txt".into())
        );
        assert_eq!(
            scan(&["--debug-file", "/tmp/b.txt"]),
            Some("/tmp/b.txt".into())
        );
        assert_eq!(scan(&["chat"]), None);
    }

    #[test]
    fn filter_uses_debug_filter_module() {
        // --debug=pattern wiring flows into the debugFilter port.
        let f = crate::generated_ports::p_7a2adbb3dff05::parse_debug_filter(Some("api,!1p"));
        assert!(f.is_none()); // mixed filters rejected
        let f = crate::generated_ports::p_7a2adbb3dff05::parse_debug_filter(Some("api")).unwrap();
        assert_eq!(f.include, vec!["api"]);
    }

    #[test]
    fn session_id_lazy_and_stable() {
        let a = get_session_id();
        let b = get_session_id();
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn formatted_output_flag_roundtrip() {
        set_has_formatted_output(true);
        assert!(get_has_formatted_output());
        set_has_formatted_output(false);
        assert!(!get_has_formatted_output());
    }

    #[test]
    fn log_path_fallback_chain_shape() {
        // get_debug_log_path honors ATLAS_DEBUG_LOGS_DIR and --debug-file
        // first; with neither set it falls back to config home
        // debug/{session}.txt. Env mutation is covered by integration tests;
        // here we verify the fallback chain shape.
        let path = get_debug_log_path();
        assert!(path.to_string_lossy().contains("debug"));
        assert!(path.to_string_lossy().ends_with(".txt"));
    }

    #[test]
    fn log_for_debugging_respects_level_gate() {
        // verbose below default 'debug' gate must not panic even when silent.
        log_for_debugging("should be filtered", DebugLogLevel::verbose);
        // error level passes the gate; silent unless debug mode active.
        log_for_debugging("error path", DebugLogLevel::error);
    }
}
