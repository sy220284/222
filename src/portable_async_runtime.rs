//! Async and queue primitives rewritten from the original 2.1.88 utility
//! sources. No vendor login, network reporting or JavaScript engine required.
#![allow(dead_code)]
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tokio::sync::{mpsc, oneshot, watch};
use uuid::Uuid;

// src/utils/agentId.ts
pub fn format_agent_id(agent_name: &str, team_name: &str) -> String {
    format!("{agent_name}@{team_name}")
}
pub fn parse_agent_id(value: &str) -> Option<(&str, &str)> {
    value.split_once('@')
}
pub fn generate_request_id(kind: &str, id: &str, timestamp_ms: i64) -> String {
    format!("{kind}-{timestamp_ms}@{id}")
}
pub fn parse_request_id(value: &str) -> Option<(String, i64, String)> {
    let (prefix, agent) = value.split_once('@')?;
    let (kind, date) = prefix.rsplit_once('-')?;
    // Upstream agentId.ts uses JavaScript parseInt(timestampStr, 10):
    // a decimal prefix is accepted even when trailing text is present.
    let decimal = date.trim_start_matches(char::is_whitespace);
    let decimal = decimal.strip_prefix('+').unwrap_or(decimal);
    let digit_count = decimal.bytes().take_while(u8::is_ascii_digit).count();
    if digit_count == 0 {
        return None;
    }
    let timestamp = decimal[..digit_count].parse::<i64>().ok()?;
    Some((kind.into(), timestamp, agent.into()))
}

// src/utils/queueProcessor.ts: priority deque, main-thread-only, mode-stable.
#[derive(Clone, Debug, PartialEq)]
pub struct QueuedInput {
    pub value: Value,
    pub mode: String,
    pub agent_id: Option<String>,
    pub uuid: String,
}
pub fn is_slash_command(command: &QueuedInput) -> bool {
    let value = match &command.value {
        Value::String(s) => Some(s.as_str()),
        Value::Array(blocks) => blocks.iter().find_map(|b| {
            (b.get("type").and_then(Value::as_str) == Some("text"))
                .then(|| b.get("text").and_then(Value::as_str))
                .flatten()
        }),
        _ => None,
    };
    value.is_some_and(|s| s.trim().starts_with('/'))
}
pub fn dequeue_ready(queue: &mut VecDeque<QueuedInput>) -> Vec<QueuedInput> {
    let Some(next) = queue.iter().find(|cmd| cmd.agent_id.is_none()).cloned() else {
        return Vec::new();
    };
    if is_slash_command(&next) || next.mode == "bash" {
        return queue
            .iter()
            .position(|cmd| cmd.agent_id.is_none())
            .and_then(|i| queue.remove(i))
            .into_iter()
            .collect();
    }
    let mut selected = Vec::new();
    let mut i = 0;
    while i < queue.len() {
        let matched = queue.get(i).is_some_and(|cmd| {
            cmd.agent_id.is_none() && !is_slash_command(cmd) && cmd.mode == next.mode
        });
        if matched {
            selected.push(queue.remove(i).unwrap());
        } else {
            i += 1;
        }
    }
    selected
}

// src/utils/sequential.ts — fair async Mutex serializes whole operation.
#[derive(Clone)]
pub struct Sequential {
    gate: Arc<tokio::sync::Mutex<()>>,
}
impl Sequential {
    pub fn new() -> Self {
        Self {
            gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
    pub async fn call<A, F, FutureResult, T>(&self, args: A, fnc: F) -> T
    where
        F: FnOnce(A) -> FutureResult,
        FutureResult: Future<Output = T>,
    {
        let _guard = self.gate.lock().await;
        fnc(args).await
    }
}
impl Default for Sequential {
    fn default() -> Self {
        Self::new()
    }
}

// src/utils/withResolvers.ts — one-shot promise/resolver.
pub fn with_resolvers<T>() -> (oneshot::Sender<T>, oneshot::Receiver<T>) {
    oneshot::channel()
}

// src/utils/abortController.ts / combinedAbortSignal.ts
#[derive(Clone, Debug)]
pub struct AbortController {
    state: Arc<watch::Sender<bool>>,
}
impl AbortController {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        Self {
            state: Arc::new(tx),
        }
    }
    pub fn abort(&self) {
        self.state.send_replace(true);
    }
    pub fn aborted(&self) -> bool {
        *self.state.borrow()
    }
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.state.subscribe()
    }
    pub fn child(&self) -> Self {
        let child = Self::new();
        if self.aborted() {
            child.abort();
            return child;
        }
        let parent = self.subscribe();
        let result = child.clone();
        tokio::spawn(async move {
            await_abort(parent).await;
            result.abort();
        });
        child
    }
}
impl Default for AbortController {
    fn default() -> Self {
        Self::new()
    }
}
async fn await_abort(mut rx: watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    while rx.changed().await.is_ok() {
        if *rx.borrow() {
            return;
        }
    }
}
pub struct CombinedAbort {
    pub signal: AbortController,
    listener: tokio::task::JoinHandle<()>,
}
impl CombinedAbort {
    pub fn new(
        primary: Option<&AbortController>,
        secondary: Option<&AbortController>,
        timeout_ms: Option<u64>,
    ) -> Self {
        let child = AbortController::new();
        if primary.is_some_and(AbortController::aborted)
            || secondary.is_some_and(AbortController::aborted)
        {
            child.abort();
        }
        let watch_a = primary.map(AbortController::subscribe);
        let watch_b = secondary.map(AbortController::subscribe);
        let copy = child.clone();
        let listener = tokio::spawn(async move {
            let wait_a = async {
                if let Some(rx) = watch_a {
                    await_abort(rx).await
                } else {
                    std::future::pending::<()>().await
                }
            };
            let wait_b = async {
                if let Some(rx) = watch_b {
                    await_abort(rx).await
                } else {
                    std::future::pending::<()>().await
                }
            };
            let wait_timeout = async {
                if let Some(ms) = timeout_ms {
                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await
                } else {
                    std::future::pending::<()>().await
                }
            };
            tokio::select! {_ = wait_a=>{},_ = wait_b=>{},_ = wait_timeout=>{}}
            copy.abort();
        });
        Self {
            signal: child,
            listener,
        }
    }
    pub fn cleanup(self) {
        self.listener.abort()
    }
}
impl Drop for CombinedAbort {
    fn drop(&mut self) {
        self.listener.abort()
    }
}

// src/utils/stream.ts: queued FIFO async stream, terminal done/error signals.
pub struct AsyncStream<T> {
    tx: mpsc::UnboundedSender<std::result::Result<T, String>>,
    rx: mpsc::UnboundedReceiver<std::result::Result<T, String>>,
    done: bool,
}
impl<T> AsyncStream<T> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            done: false,
        }
    }
    pub fn enqueue(&self, value: T) {
        let _ = self.tx.send(Ok(value));
    }
    pub fn error(&mut self, message: impl Into<String>) {
        let _ = self.tx.send(Err(message.into()));
        self.done = true;
    }
    pub fn done(&mut self) {
        self.done = true
    }
    pub async fn next(&mut self) -> Option<std::result::Result<T, String>> {
        if let Ok(v) = self.rx.try_recv() {
            return Some(v);
        }
        if self.done {
            return None;
        }
        self.rx.recv().await
    }
}
impl<T> Default for AsyncStream<T> {
    fn default() -> Self {
        Self::new()
    }
}

// src/utils/sdkEventQueue.ts: bounded event bus and drain with new UUIDs.
#[derive(Debug)]
pub struct SdkEventQueue {
    messages: VecDeque<Value>,
    enabled: bool,
}
impl SdkEventQueue {
    pub fn new(enabled: bool) -> Self {
        Self {
            messages: VecDeque::new(),
            enabled,
        }
    }
    pub fn enqueue(&mut self, event: Value) {
        if !self.enabled {
            return;
        }
        if self.messages.len() >= 1000 {
            self.messages.pop_front();
        }
        self.messages.push_back(event);
    }
    pub fn drain(&mut self, session_id: &str) -> Vec<Value> {
        self.messages
            .drain(..)
            .filter_map(|event| {
                let Value::Object(mut obj) = event else {
                    return None;
                };
                obj.insert("uuid".into(), json!(Uuid::new_v4().to_string()));
                obj.insert("session_id".into(), json!(session_id));
                Some(Value::Object(obj))
            })
            .collect()
    }
    pub fn len(&self) -> usize {
        self.messages.len()
    }
}

// src/utils/userPromptKeywords.ts. JavaScript /\\b/ uses ASCII
// [A-Za-z0-9_] word boundaries without the /u flag. Rust regex defaults
// to Unicode boundaries, which silently misses "éwtf" and "中文go on".
fn matches_js_word_boundaries(input: &str, pattern: &Regex) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    pattern.find_iter(input).any(|found| {
        let before = input[..found.start()].chars().next_back();
        let after = input[found.end()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

pub fn matches_negative_keyword(input: &str) -> bool {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"(wtf|wth|ffs|omfg|shit(ty|tiest)?|dumbass|horrible|awful|piss(ed|ing)? off|piece of (shit|crap|junk)|what the (fuck|hell)|fucking? (broken|useless|terrible|awful|horrible)|fuck you|screw (this|you)|so frustrating|this sucks|damn it)").expect("source negative keyword regex")
    });
    matches_js_word_boundaries(&input.to_lowercase(), pattern)
}

pub fn matches_keep_going_keyword(input: &str) -> bool {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();
    if trimmed == "continue" {
        return true;
    }
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"(keep going|go on)").expect("source continuation keyword regex")
    });
    matches_js_word_boundaries(trimmed, pattern)
}

// src/utils/sessionUrl.ts — SDK resume selectors: UUID, JSONL, URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSessionIdentifier {
    pub session_id: String,
    pub ingress_url: Option<String>,
    pub is_url: bool,
    pub jsonl_file: Option<String>,
    pub is_jsonl_file: bool,
}
pub fn parse_session_identifier(id: &str) -> Option<ParsedSessionIdentifier> {
    if id.to_ascii_lowercase().ends_with(".jsonl") {
        return Some(ParsedSessionIdentifier {
            session_id: Uuid::new_v4().to_string(),
            ingress_url: None,
            is_url: false,
            jsonl_file: Some(id.into()),
            is_jsonl_file: true,
        });
    }
    if crate::portable_source_utils::validate_uuid(&json!(id)).is_some() {
        return Some(ParsedSessionIdentifier {
            session_id: id.into(),
            ingress_url: None,
            is_url: false,
            jsonl_file: None,
            is_jsonl_file: false,
        });
    }
    let url = reqwest::Url::parse(id).ok()?;
    Some(ParsedSessionIdentifier {
        session_id: Uuid::new_v4().to_string(),
        ingress_url: Some(url.to_string()),
        is_url: true,
        jsonl_file: None,
        is_jsonl_file: false,
    })
}

// src/utils/toolPool.ts — stable partition sorting with initial tool priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolEntry {
    pub name: String,
    pub mcp: bool,
}
pub fn is_pr_activity_subscription_tool(name: &str) -> bool {
    name.ends_with("subscribe_pr_activity") || name.ends_with("unsubscribe_pr_activity")
}
pub fn merge_and_filter_tools(
    initial: Vec<ToolEntry>,
    assembled: Vec<ToolEntry>,
    coordinator: bool,
    allowed: &[String],
) -> Vec<ToolEntry> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tool in initial.into_iter().chain(assembled) {
        if seen.insert(tool.name.clone()) {
            result.push(tool)
        }
    }
    result.sort_by(|a, b| a.mcp.cmp(&b.mcp).then_with(|| a.name.cmp(&b.name)));
    if coordinator {
        result.retain(|tool| {
            allowed.contains(&tool.name) || is_pr_activity_subscription_tool(&tool.name)
        });
    }
    result
}

// src/utils/fileReadCache.ts: mtime-based invalidation and FIFO eviction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileText {
    pub content: String,
    pub encoding: String,
}
#[derive(Debug)]
struct FileCacheEntry {
    data: FileText,
    mtime: Option<SystemTime>,
}
#[derive(Debug, Default)]
pub struct FileReadCache {
    cache: HashMap<PathBuf, FileCacheEntry>,
    order: VecDeque<PathBuf>,
}
impl FileReadCache {
    pub fn read_file(&mut self, path: &Path) -> Result<FileText> {
        let meta = match fs::metadata(path) {
            Ok(s) => s,
            Err(e) => {
                self.invalidate(path);
                return Err(e.into());
            }
        };
        let mtime = meta.modified().ok();
        if let Some(cached) = self.cache.get(path) {
            if cached.mtime == mtime {
                return Ok(cached.data.clone());
            }
        }
        let bytes = fs::read(path)?;
        let (content, encoding) = if bytes.starts_with(&[0xff, 0xfe]) {
            let chars = bytes[2..]
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect::<Vec<_>>();
            (String::from_utf16(&chars)?, "utf16le".to_owned())
        } else if bytes.starts_with(&[0xfe, 0xff]) {
            let chars = bytes[2..]
                .chunks_exact(2)
                .map(|p| u16::from_be_bytes([p[0], p[1]]))
                .collect::<Vec<_>>();
            (String::from_utf16(&chars)?, "utf16be".to_owned())
        } else {
            (
                String::from_utf8(bytes)?
                    .trim_start_matches('\u{feff}')
                    .to_string(),
                "utf8".to_owned(),
            )
        };
        let data = FileText {
            content: content.replace("\r\n", "\n"),
            encoding,
        };
        if !self.cache.contains_key(path) {
            self.order.push_back(path.to_path_buf())
        }
        self.cache.insert(
            path.to_path_buf(),
            FileCacheEntry {
                data: data.clone(),
                mtime,
            },
        );
        while self.cache.len() > 1000 {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            } else {
                break;
            }
        }
        Ok(data)
    }
    pub fn invalidate(&mut self, path: &Path) {
        self.cache.remove(path);
        self.order.retain(|x| x != path);
    }
    pub fn clear(&mut self) {
        self.cache.clear();
        self.order.clear()
    }
    pub fn stats(&self) -> Vec<PathBuf> {
        self.order.iter().cloned().collect()
    }
}

// src/utils/lockfile.ts — platform file locking via RAII.
pub struct FileLock {
    file: File,
}
impl FileLock {
    pub fn exclusive(path: &Path) -> Result<Self> {
        use fs2::FileExt;
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        file.lock_exclusive()?;
        Ok(Self { file })
    }
    pub fn checked(path: &Path) -> Result<bool> {
        use fs2::FileExt;
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock_exclusive() {
            Ok(()) => {
                FileExt::unlock(&file)?;
                Ok(false)
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
            Err(err) => Err(err.into()),
        }
    }
}
impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

// src/utils/workloadContext.ts — async task-local workload boundary.
tokio::task_local! {static WORKLOAD:Option<String>;}
pub async fn run_with_workload<R, F: Future<Output = R>>(workload: Option<String>, future: F) -> R {
    WORKLOAD.scope(workload, future).await
}
pub fn get_workload() -> Option<String> {
    WORKLOAD.try_with(Clone::clone).ok().flatten()
}

// src/utils/idleTimeout.ts: no process::exit in library; host owns shutdown.
#[derive(Clone, Debug)]
pub struct IdleTimeout {
    delay_ms: u64,
    started: Option<std::time::Instant>,
}
impl IdleTimeout {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            started: None,
        }
    }
    pub fn start(&mut self) {
        self.started = Some(std::time::Instant::now())
    }
    pub fn stop(&mut self) {
        self.started = None
    }
    pub fn expired(&self, is_idle: bool) -> bool {
        self.started
            .is_some_and(|s| is_idle && s.elapsed().as_millis() >= u128::from(self.delay_ms))
    }
}

// src/utils/bufferedWriter.ts — bounded buffer, explicit flush and disposal.
pub struct BufferedWriter<F: Fn(&str)> {
    buffer: Vec<String>,
    bytes: usize,
    write: F,
    max_items: usize,
    max_bytes: usize,
    immediate: bool,
}
impl<F: Fn(&str)> BufferedWriter<F> {
    pub fn new(write: F, max_items: usize, max_bytes: usize, immediate: bool) -> Self {
        Self {
            write,
            buffer: Vec::new(),
            bytes: 0,
            max_items,
            max_bytes,
            immediate,
        }
    }
    pub fn push(&mut self, value: &str) {
        if self.immediate {
            (self.write)(value);
            return;
        }
        self.bytes += value.len();
        self.buffer.push(value.into());
        if self.buffer.len() >= self.max_items || self.bytes >= self.max_bytes {
            self.flush()
        }
    }
    pub fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        (self.write)(&self.buffer.concat());
        self.buffer.clear();
        self.bytes = 0;
    }
}
impl<F: Fn(&str)> Drop for BufferedWriter<F> {
    fn drop(&mut self) {
        self.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_mode_filter_and_agent_skip() {
        let mk = |s: &str, mode: &str, agent: Option<&str>| QueuedInput {
            value: json!(s),
            mode: mode.into(),
            agent_id: agent.map(str::to_owned),
            uuid: s.into(),
        };
        let mut q = VecDeque::from([
            mk("notify", "prompt", Some("worker")),
            mk("hello", "prompt", None),
            mk("/clear", "prompt", None),
            mk("world", "prompt", None),
            mk("echo hi", "bash", None),
        ]);
        let first = dequeue_ready(&mut q);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].uuid, "hello");
        assert_eq!(first[1].uuid, "world");
        assert_eq!(dequeue_ready(&mut q)[0].uuid, "/clear");
        assert_eq!(q.len(), 2);
    }
    #[test]
    fn resume_identifier_preserves_source_url_jsonl_and_uuid_flags() {
        let file = parse_session_identifier(r"C:\Users\User\chat.JSONL").unwrap();
        assert!(file.is_jsonl_file);
        assert!(!file.is_url);
        assert_eq!(file.jsonl_file.as_deref(), Some(r"C:\Users\User\chat.JSONL"));
        assert!(file.ingress_url.is_none());
        assert!(Uuid::parse_str(&file.session_id).is_ok());

        let plain = "550e8400-e29b-41d4-a716-446655440000";
        let uuid = parse_session_identifier(plain).unwrap();
        assert_eq!(uuid.session_id, plain);
        assert!(!uuid.is_url);
        assert!(!uuid.is_jsonl_file);
        assert!(uuid.jsonl_file.is_none());
        assert!(uuid.ingress_url.is_none());

        let remote = parse_session_identifier(
            "HTTPS://EXAMPLE.COM/v1/session_ingress/session?id=1#hash"
        )
        .unwrap();
        assert!(remote.is_url);
        assert!(!remote.is_jsonl_file);
        assert_eq!(
            remote.ingress_url.as_deref(),
            Some("https://example.com/v1/session_ingress/session?id=1#hash")
        );
        assert!(Uuid::parse_str(&remote.session_id).is_ok());
        assert!(parse_session_identifier("not a url or uuid").is_none());
    }

    #[test]
    fn user_prompt_keyword_boundaries_match_source_javascript_ascii_rules() {
        for (input, negative, keep_going) in [
            ("éwtf!", true, false),
            ("wtfé", true, false),
            ("wtf_", false, false),
            ("wtf👌", true, false),
            ("继续go on结束", false, true),
            ("foo go on_bar", false, false),
            ("GO ON!", false, true),
            ("Keep Going", false, true),
            ("discontinue", false, false),
            ("Foo go on.", false, true),
        ] {
            assert_eq!(
                matches_negative_keyword(input),
                negative,
                "negative: {input}"
            );
            assert_eq!(
                matches_keep_going_keyword(input),
                keep_going,
                "keep going: {input}"
            );
        }
        assert!(matches_keep_going_keyword("  CONTINUE  "));
        assert!(!matches_keep_going_keyword("Continue the task"));
        assert!(!matches_negative_keyword("awfulness"));
        assert!(matches_negative_keyword("what the hell!"));
    }

    #[test]
    fn identity_and_session_parsing() {
        let id = format_agent_id("a", "team");
        assert_eq!(parse_agent_id(&id), Some(("a", "team")));
        let request = generate_request_id("shutdown", &id, 1000);
        assert_eq!(
            parse_request_id(&request),
            Some(("shutdown".into(), 1000, id))
        );
        assert!(parse_session_identifier("/tmp/history.JSONL")
            .unwrap()
            .jsonl_file
            .is_some());
        assert!(parse_session_identifier("https://example.org/a")
            .unwrap()
            .ingress_url
            .is_some());
        assert!(parse_session_identifier("not-a-session").is_none());
        assert!(matches_keep_going_keyword("Please keep going"));
        assert!(!matches_keep_going_keyword("Continue the process"));
    }
    #[test]
    fn request_id_parse_matches_js_parse_int_prefix_and_rejects_nonnumeric() {
        assert_eq!(
            parse_request_id("plan-123tail@worker@team"),
            Some(("plan".into(), 123, "worker@team".into()))
        );
        assert_eq!(
            parse_request_id("stop-+17junk@worker"),
            Some(("stop".into(), 17, "worker".into()))
        );
        assert_eq!(
            parse_request_id("step-0x10@worker"),
            Some(("step".into(), 0, "worker".into()))
        );
        assert_eq!(
            parse_request_id("step- 42.8@worker"),
            Some(("step".into(), 42, "worker".into()))
        );
        assert!(parse_request_id("step-abc123@worker").is_none());
        assert!(parse_request_id("step-123-no-at-sign").is_none());
    }
    #[tokio::test]
    async fn abort_propagation_sequential_workload() {
        let parent = AbortController::new();
        let child = parent.child();
        assert!(!child.aborted());
        parent.abort();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !child.aborted() {
                tokio::task::yield_now().await
            }
        })
        .await
        .unwrap();
        let seq = Sequential::new();
        let value = seq.call(7, |n| async move { n * 2 }).await;
        assert_eq!(value, 14);
        let result = run_with_workload(Some("cron".into()), async { get_workload() }).await;
        assert_eq!(result, Some("cron".into()));
        let result = run_with_workload(None, async { get_workload() }).await;
        assert_eq!(result, None);
    }
    #[test]
    fn sdk_queue_and_pool() {
        let mut q = SdkEventQueue::new(true);
        for i in 0..1002 {
            q.enqueue(json!({"type":"system","i":i}))
        }
        assert_eq!(q.len(), 1000);
        let drained = q.drain("session");
        assert_eq!(drained[0]["i"], 2);
        assert_eq!(drained[0]["session_id"], "session");
        let merged = merge_and_filter_tools(
            vec![ToolEntry {
                name: "Read".into(),
                mcp: false,
            }],
            vec![
                ToolEntry {
                    name: "Read".into(),
                    mcp: false,
                },
                ToolEntry {
                    name: "mcp__a".into(),
                    mcp: true,
                },
            ],
            false,
            &[],
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].name, "Read");
    }
}
