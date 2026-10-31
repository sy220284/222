//! Source parity: utils/hooks/fileChangedWatcher.ts.
//!
//! The original starts a chokidar watcher for `FileChanged` hook rules only.
//! Watch paths come from each rule's matcher field, pipe-separated file names
//! (e.g. ".envrc|.env") resolved against the session cwd. Detected
//! change/add/unlink events run the matching hooks with a FileChanged input;
//! hook system messages surface at the next input boundary in the terminal
//! session instead of interrupting the active prompt line.
//!
//! The offline toolchain has no filesystem-event crate available, so the
//! watcher polls mtimes on a fixed interval; the observable contract (event
//! kinds, hook input shape, no hooks configured -> no watcher) matches the
//! source implementation.

use crate::hooks::{HookEvent, HookOutcome, HookRule, HookRunner};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

/// Poll cadence for the mtime snapshot loop.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Resolve watch paths from FileChanged rule matchers, mirroring
/// resolveWatchPaths: matcher is a pipe-separated file-name list, relative
/// names resolve against the session cwd, absolute paths stay untouched.
pub fn resolve_watch_paths(rules: &[HookRule], workspace: &Path) -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    for rule in rules {
        if rule.event != HookEvent::FileChanged {
            continue;
        }
        let Some(matcher) = rule.matcher.as_deref() else {
            continue;
        };
        for name in matcher.split('|').map(str::trim).filter(|s| !s.is_empty()) {
            let candidate = Path::new(name);
            paths.insert(if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                workspace.join(name)
            });
        }
    }
    paths.into_iter().collect()
}

/// Combine matcher paths and the latest dynamic watchPaths hook output.
/// An empty hook output does not replace a previous dynamic set; callers only
/// apply this function after a hook returns one or more explicit watch paths.
fn combined_watch_paths(
    static_paths: &[PathBuf],
    dynamic_paths: &[String],
    workspace: &Path,
) -> Vec<PathBuf> {
    let mut combined: BTreeSet<PathBuf> = static_paths.iter().cloned().collect();
    for path in dynamic_paths {
        let raw = Path::new(path);
        combined.insert(if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            workspace.join(raw)
        });
    }
    combined.into_iter().collect()
}

/// One snapshot entry per watched path: existence plus modification time.
type Snapshot = HashMap<PathBuf, Option<SystemTime>>;

fn snapshot(paths: &[PathBuf]) -> Snapshot {
    paths
        .iter()
        .map(|path| {
            let state = std::fs::metadata(path)
                .ok()
                .and_then(|meta| meta.modified().ok());
            (path.clone(), state)
        })
        .collect()
}

/// Diff two snapshots into change/add/unlink events, mirroring the chokidar
/// `change`/`add`/`unlink` handlers in the source watcher.
fn diff_events(prev: &Snapshot, next: &Snapshot) -> Vec<(PathBuf, &'static str)> {
    let mut events = Vec::new();
    for (path, previous) in prev {
        match (previous.as_ref(), next.get(path).copied().flatten()) {
            (Some(before), Some(after)) if *before != after => {
                events.push((path.clone(), "change"));
            }
            (Some(_), None) => events.push((path.clone(), "unlink")),
            // Static matchers stay in both snapshots even when the file was
            // missing at startup. A missing -> present transition is an add,
            // just like chokidar's 'add' handler after ignoreInitial.
            (None, Some(_)) => events.push((path.clone(), "add")),
            _ => {}
        }
    }
    for (path, current) in next {
        if !prev.contains_key(path) && current.is_some() {
            events.push((path.clone(), "add"));
        }
    }
    events
}

fn file_changed_input(path: &Path, event: &str) -> Value {
    json!({
        "file_path": path.to_string_lossy(),
        "event": event,
    })
}

/// Messages surfaced to the user between input rounds.
fn outcome_messages(outcome: &HookOutcome) -> Vec<String> {
    let mut messages = Vec::new();
    if let Some(reason) = outcome.blocking_reason.as_deref() {
        messages.push(reason.to_owned());
    }
    messages.extend(outcome.additional_context.iter().cloned());
    messages.extend(outcome.non_blocking_errors.iter().cloned());
    messages
}

/// Live watcher handle; abort on session teardown so no poll task outlives
/// the conversation (source registerCleanup(dispose)).
pub struct FileChangedWatcher {
    handle: JoinHandle<()>,
    pub messages: UnboundedReceiver<String>,
}

impl FileChangedWatcher {
    pub fn abort(self) {
        self.handle.abort();
    }
}

impl Drop for FileChangedWatcher {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Start polling for FileChanged hook rules. Returns None when no rule
/// contributes a watch path, matching the source early-return.
pub fn spawn(rules: &[HookRule], workspace: &Path) -> anyhow::Result<Option<FileChangedWatcher>> {
    let static_paths = resolve_watch_paths(rules, workspace);
    if static_paths.is_empty() {
        return Ok(None);
    }
    let workspace = workspace.to_path_buf();
    let hook_rules: Vec<HookRule> = rules
        .iter()
        .filter(|rule| rule.event == HookEvent::FileChanged)
        .cloned()
        .collect();
    let runner = HookRunner::new(&hook_rules)?
        .with_workspace(workspace.to_path_buf())
        .with_project_root(workspace.to_path_buf());
    let (tx, rx) = unbounded_channel::<String>();
    let handle = tokio::spawn(async move {
        let mut watch_paths = static_paths.clone();
        let mut previous = snapshot(&watch_paths);
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let current = snapshot(&watch_paths);
            let mut replacement_paths = None;
            for (path, event) in diff_events(&previous, &current) {
                match runner
                    .run(
                        HookEvent::FileChanged,
                        None,
                        file_changed_input(&path, event),
                    )
                    .await
                {
                    Ok(outcome) => {
                        if !outcome.watch_paths.is_empty() {
                            let candidate = combined_watch_paths(
                                &static_paths,
                                &outcome.watch_paths,
                                &workspace,
                            );
                            if candidate != watch_paths {
                                replacement_paths = Some(candidate);
                            }
                        }
                        for message in outcome_messages(&outcome) {
                            if !tx.send(message).is_ok() {
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(format!("FileChanged hook failed: {error:#}"));
                    }
                }
            }
            if let Some(paths) = replacement_paths {
                // Original restartWatching uses ignoreInitial. Seeding the
                // new watcher snapshot avoids replaying existing files as adds.
                watch_paths = paths;
                previous = snapshot(&watch_paths);
            } else {
                previous = current;
            }
        }
    });
    Ok(Some(FileChangedWatcher {
        handle,
        messages: rx,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(matcher: Option<&str>) -> HookRule {
        HookRule {
            async_background: false,
            async_rewake: false,
            event: HookEvent::FileChanged,
            matcher: matcher.map(str::to_owned),
            if_condition: None,
            plugin_root: None,
            command: "echo changed".into(),
            shell: None,
            timeout_seconds: 0,
        }
    }

    #[test]
    fn watch_paths_come_from_matcher_pipe_lists() {
        let workspace = Path::new("/tmp/proj");
        let rules = vec![
            rule(Some(".envrc|.env")),
            rule(Some("/absolute/path")),
            rule(None),
            rule(Some("  ")),
        ];
        let paths = resolve_watch_paths(&rules, workspace);
        assert_eq!(paths.len(), 3);
        assert!(paths.contains(&workspace.join(".envrc")));
        assert!(paths.contains(&workspace.join(".env")));
        assert!(paths.contains(&PathBuf::from("/absolute/path")));
    }

    #[test]
    fn no_watch_paths_means_no_watcher() {
        assert!(spawn(&[rule(None)], Path::new("/tmp")).unwrap().is_none());
        assert!(spawn(&[], Path::new("/tmp")).unwrap().is_none());
    }

    #[test]
    fn other_events_never_contribute_watch_paths() {
        let mut unrelated = rule(Some(".env"));
        unrelated.event = HookEvent::PreToolUse;
        assert!(resolve_watch_paths(&[unrelated], Path::new("/tmp")).is_empty());
    }

    #[test]
    fn dynamic_watch_paths_replace_previous_set_but_keep_static_matchers() {
        let workspace = Path::new("/tmp/work");
        let static_paths = vec![workspace.join(".env")];
        let first = combined_watch_paths(
            &static_paths,
            &["generated/app.json".into(), ".env".into()],
            workspace,
        );
        assert_eq!(
            first,
            vec![workspace.join(".env"), workspace.join("generated/app.json")]
        );
        let replacement =
            combined_watch_paths(&static_paths, &["/tmp/other/config.json".into()], workspace);
        assert_eq!(
            replacement,
            vec![
                PathBuf::from("/tmp/other/config.json"),
                workspace.join(".env")
            ]
        );
    }

    #[test]
    fn diff_reports_change_add_unlink() {
        let a = PathBuf::from("/tmp/a");
        let b = PathBuf::from("/tmp/b");
        let t1 = SystemTime::UNIX_EPOCH;
        let t2 = t1 + Duration::from_secs(1);
        let prev = Snapshot::from([(a.clone(), Some(t1)), (b.clone(), Some(t1))]);
        // a modified, b removed, c added
        let c = PathBuf::from("/tmp/c");
        let next = Snapshot::from([(a.clone(), Some(t2)), (c.clone(), Some(t1))]);
        let events = diff_events(&prev, &next);
        assert!(events.contains(&(a, "change")));
        assert!(events.contains(&(b, "unlink")));
        assert!(events.contains(&(c, "add")));
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn initially_missing_watched_file_triggers_add_and_recreate_triggers_add_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        let missing = snapshot(&[path.clone()]);
        assert!(diff_events(&missing, &missing).is_empty());

        std::fs::write(&path, "value=1").unwrap();
        let created = snapshot(&[path.clone()]);
        assert_eq!(diff_events(&missing, &created), vec![(path.clone(), "add")]);

        std::fs::remove_file(&path).unwrap();
        let removed = snapshot(&[path.clone()]);
        assert_eq!(
            diff_events(&created, &removed),
            vec![(path.clone(), "unlink")]
        );

        std::fs::write(&path, "value=2").unwrap();
        assert_eq!(
            diff_events(&removed, &snapshot(&[path.clone()])),
            vec![(path, "add")]
        );
    }

    #[test]
    fn unchanged_snapshot_yields_no_events() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a");
        std::fs::write(&p, "content").unwrap();
        let prev = snapshot(&[p.clone()]);
        // Unmodified path: the second poll must not report any event.
        assert!(diff_events(&prev, &snapshot(&[p])).is_empty());
    }

    #[test]
    fn input_carries_path_and_event() {
        let input = file_changed_input(Path::new("/tmp/.envrc"), "change");
        assert_eq!(input["event"], "change");
        assert_eq!(input["file_path"], "/tmp/.envrc");
    }

    #[test]
    fn outcome_messages_collect_blocking_context_and_errors() {
        let outcome = HookOutcome {
            blocking_reason: Some("blocked".into()),
            blocking_reasons: vec!["blocked".into()],
            additional_context: vec!["note".into()],
            initial_user_message: None,
            watch_paths: Vec::new(),
            updated_input: None,
            permission_decision: None,
            updated_mcp_tool_output: None,
            prevent_continuation: false,
            stop_reason: None,
            non_blocking_errors: vec!["warn".into()],
        };
        let messages = outcome_messages(&outcome);
        assert_eq!(messages, vec!["blocked", "note", "warn"]);
    }
}
