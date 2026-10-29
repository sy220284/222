//! Portable, provider-neutral rewrites of independent 2.1.88 source utilities.
//! Each public group documents its original path. These are implementations,
//! not evidence of final whole-file parity; upstream callers are migrated
//! separately.
#![allow(dead_code)]
use anyhow::{bail, Result};
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use uuid::Uuid;

// src/utils/CircularBuffer.ts
#[derive(Debug, Clone)]
pub struct CircularBuffer<T> {
    capacity: usize,
    buffer: VecDeque<T>,
}
impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            bail!("CircularBuffer capacity must be greater than zero")
        }
        Ok(Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        })
    }
    pub fn add(&mut self, value: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(value);
    }
    pub fn add_all(&mut self, values: impl IntoIterator<Item = T>) {
        for value in values {
            self.add(value);
        }
    }
    pub fn get_recent(&self, count: usize) -> Vec<&T> {
        self.buffer
            .iter()
            .skip(self.buffer.len().saturating_sub(count))
            .collect()
    }
    pub fn to_array(&self) -> Vec<&T> {
        self.buffer.iter().collect()
    }
    pub fn clear(&mut self) {
        self.buffer.clear()
    }
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

// src/utils/binaryCheck.ts and src/utils/findExecutable.ts
static BINARY_CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
pub fn is_binary_installed(command: &str) -> bool {
    let name = command.trim();
    if name.is_empty() {
        return false;
    }
    let cache = BINARY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(value) = cache.lock().unwrap().get(name).copied() {
        return value;
    }
    let exists = crate::shell_runtime::which(name).is_some();
    cache.lock().unwrap().insert(name.into(), exists);
    exists
}
pub fn clear_binary_cache() {
    if let Some(cache) = BINARY_CACHE.get() {
        cache.lock().unwrap().clear();
    }
}
pub fn find_executable(name: &str) -> Option<PathBuf> {
    crate::shell_runtime::which(name)
}

// src/utils/contentArray.ts
pub fn insert_block_after_tool_results(content: &mut Vec<Value>, block: Value) {
    let after = content
        .iter()
        .rposition(|v| v.get("type").and_then(Value::as_str) == Some("tool_result"));
    match after {
        Some(index) => {
            let position = index + 1;
            content.insert(position, block);
            if position == content.len() - 1 {
                content.push(json!({"type":"text","text":"."}));
            }
        }
        None => content.insert(content.len().saturating_sub(1), block),
    }
}

// src/utils/hash.ts (Node fallback; stable UTF-16 DJB2)
pub fn djb2_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |hash, c| {
        hash.wrapping_mul(31).wrapping_add(i32::from(c))
    })
}
pub fn hash_content(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}
pub fn hash_pair(a: &str, b: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(a.as_bytes());
    hasher.update([0]);
    hasher.update(b.as_bytes());
    hex(&hasher.finalize())
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut text, "{b:02x}");
    }
    text
}

// src/utils/cachePaths.ts (provider-neutral application cache root)
pub fn sanitize_cache_component(text: &str) -> String {
    let replaced = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    if replaced.len() <= 200 {
        return replaced;
    }
    let hash = (djb2_hash(text) as i64).abs();
    format!("{}-{}", &replaced[..200], to_radix36(hash as u64))
}
fn to_radix36(mut number: u64) -> String {
    if number == 0 {
        return "0".into();
    }
    let mut result = Vec::new();
    while number > 0 {
        let d = (number % 36) as u8;
        result.push(if d < 10 { b'0' + d } else { b'a' + (d - 10) });
        number /= 36;
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}
pub fn cache_project_path(cache_root: &Path, workspace: &Path, category: &str) -> PathBuf {
    cache_root
        .join(sanitize_cache_component(&workspace.to_string_lossy()))
        .join(category)
}
pub fn cache_mcp_log_path(cache_root: &Path, workspace: &Path, server: &str) -> PathBuf {
    cache_project_path(
        cache_root,
        workspace,
        &format!("mcp-logs-{}", sanitize_cache_component(server)),
    )
}

// src/utils/peerAddress.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerScheme {
    Uds,
    Bridge,
    Other,
}
pub fn parse_peer_address(value: &str) -> (PeerScheme, &str) {
    if let Some(to) = value.strip_prefix("uds:") {
        return (PeerScheme::Uds, to);
    }
    if let Some(to) = value.strip_prefix("bridge:") {
        return (PeerScheme::Bridge, to);
    }
    if value.starts_with('/') {
        return (PeerScheme::Uds, value);
    }
    (PeerScheme::Other, value)
}

// src/utils/sandbox/sandbox-ui-utils.ts
pub fn remove_sandbox_violation_tags(text: &str) -> String {
    let regex = Regex::new(r"(?s)<sandbox_violations>.*?</sandbox_violations>").unwrap();
    regex.replace_all(text, "").into_owned()
}

// src/utils/xdg.ts
#[derive(Clone, Debug, Default)]
pub struct XdgOptions {
    pub env: HashMap<String, String>,
    pub home: PathBuf,
}
impl XdgOptions {
    pub fn from_environment() -> Self {
        Self {
            env: std::env::vars().collect(),
            home: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
        }
    }
    pub fn state_home(&self) -> PathBuf {
        self.env
            .get("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home.join(".local/state"))
    }
    pub fn cache_home(&self) -> PathBuf {
        self.env
            .get("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home.join(".cache"))
    }
    pub fn data_home(&self) -> PathBuf {
        self.env
            .get("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home.join(".local/share"))
    }
    pub fn user_bin_dir(&self) -> PathBuf {
        self.home.join(".local/bin")
    }
}

// src/utils/cliArgs.ts
pub fn eager_parse_cli_flag(flag: &str, argv: &[String]) -> Option<String> {
    for (i, arg) in argv.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(flag).and_then(|v| v.strip_prefix('=')) {
            return Some(value.into());
        }
        if arg == flag {
            if let Some(v) = argv.get(i + 1) {
                return Some(v.clone());
            }
        }
    }
    None
}
pub fn extract_args_after_double_dash(command: &str, args: &[String]) -> (String, Vec<String>) {
    if command == "--" {
        if let Some((first, rest)) = args.split_first() {
            return (first.clone(), rest.to_vec());
        }
    }
    (command.into(), args.to_vec())
}

// src/utils/uuid.ts and src/utils/crypto.ts
pub fn validate_uuid(value: &Value) -> Option<&str> {
    let value = value.as_str()?;
    // The original UUID test accepts any hex hyphenated UUID, including non-RFC versions.
    let pat = Regex::new(r"(?i)^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$").unwrap();
    pat.is_match(value).then_some(value)
}
pub fn create_agent_id(label: Option<&str>) -> String {
    let id = Uuid::new_v4().simple().to_string();
    let id = &id[..16];
    match label {
        Some(name) if !name.is_empty() => format!("a{name}-{id}"),
        _ => format!("a{id}"),
    }
}
pub fn random_uuid() -> String {
    Uuid::new_v4().to_string()
}

// src/utils/taggedId.ts
pub fn to_tagged_id(tag: &str, uuid: &str) -> Result<String> {
    let hex = uuid.replace('-', "");
    if hex.len() != 32 {
        bail!("Invalid UUID hex length: {}", hex.len())
    }
    let mut n = u128::from_str_radix(&hex, 16)?;
    const CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut result = [b'1'; 22];
    for i in (0..22).rev() {
        if n == 0 {
            break;
        }
        result[i] = CHARS[(n % 58) as usize];
        n /= 58;
    }
    Ok(format!("{tag}_01{}", std::str::from_utf8(&result)?))
}

// src/utils/tempfile.ts
pub fn generate_temp_file_path(
    prefix: &str,
    extension: &str,
    content_hash: Option<&str>,
) -> PathBuf {
    let id = content_hash
        .map(|s| hash_content(s)[..16].to_string())
        .unwrap_or_else(random_uuid);
    std::env::temp_dir().join(format!("{prefix}-{id}{extension}"))
}

// src/utils/timeouts.ts
fn decimal_prefix(raw: &str) -> Option<u64> {
    let raw = raw.trim_start();
    let digits = raw.strip_prefix('+').unwrap_or(raw);
    if digits.starts_with('-') {
        return None;
    }
    let text = digits
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if text.is_empty() {
        None
    } else {
        text.parse::<u64>().ok()
    }
}
pub fn default_bash_timeout_ms(env: &HashMap<String, String>) -> u64 {
    env.get("BASH_DEFAULT_TIMEOUT_MS")
        .and_then(|s| decimal_prefix(s))
        .filter(|n| *n > 0)
        .unwrap_or(120_000)
}
pub fn max_bash_timeout_ms(env: &HashMap<String, String>) -> u64 {
    env.get("BASH_MAX_TIMEOUT_MS")
        .and_then(|s| decimal_prefix(s))
        .filter(|n| *n > 0)
        .unwrap_or(600_000)
        .max(default_bash_timeout_ms(env))
}

// src/utils/keyboardShortcuts.ts
pub fn macos_option_char(value: &str) -> Option<&'static str> {
    match value {
        "†" => Some("alt+t"),
        "π" => Some("alt+p"),
        "ø" => Some("alt+o"),
        _ => None,
    }
}

// src/utils/messagePredicates.ts
pub fn is_human_turn(message: &Value) -> bool {
    message.get("type").and_then(Value::as_str) == Some("user")
        && message.get("isMeta").and_then(Value::as_bool) != Some(true)
        && message.get("toolUseResult").is_none()
}

// src/utils/systemPromptType.ts
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemPrompt(Vec<String>);
impl SystemPrompt {
    pub fn new(sections: Vec<String>) -> Self {
        Self(sections)
    }
    pub fn sections(&self) -> &[String] {
        &self.0
    }
}

// src/utils/configConstants.ts
pub const NOTIFICATION_CHANNELS: &[&str] = &[
    "auto",
    "iterm2",
    "iterm2_with_bell",
    "terminal_bell",
    "kitty",
    "ghostty",
    "notifications_disabled",
];
pub const EDITOR_MODES: &[&str] = &["normal", "vim"];
pub const TEAMMATE_MODES: &[&str] = &["auto", "tmux", "in-process"];

// src/utils/tokenBudget.ts
fn budget_re() -> (Regex, Regex, Regex) {
    (
        Regex::new(r"(?i)^\s*\+(\d+(?:\.\d+)?)\s*([kmb])\b").unwrap(),
        Regex::new(r"(?i)\s\+(\d+(?:\.\d+)?)\s*([kmb])\s*[.!?]?\s*$").unwrap(),
        Regex::new(r"(?i)\b(?:use|spend)\s+(\d+(?:\.\d+)?)\s*([kmb])\s*tokens?\b").unwrap(),
    )
}
fn token_match(caps: regex::Captures<'_>) -> Option<f64> {
    let number = caps.get(1)?.as_str().parse::<f64>().ok()?;
    let multiplier = match caps.get(2)?.as_str().to_ascii_lowercase().as_str() {
        "k" => 1_000.,
        "m" => 1_000_000.,
        "b" => 1_000_000_000.,
        _ => return None,
    };
    Some(number * multiplier)
}
pub fn parse_token_budget(text: &str) -> Option<f64> {
    let (start, end, verbose) = budget_re();
    start
        .captures(text)
        .or_else(|| end.captures(text))
        .or_else(|| verbose.captures(text))
        .and_then(token_match)
}
fn utf16_index(text: &str, index: usize) -> usize {
    text.get(..index).unwrap_or("").encode_utf16().count()
}
pub fn find_token_budget_positions(text: &str) -> Vec<(usize, usize)> {
    let (start, end, verbose) = budget_re();
    let mut positions = Vec::new();
    if let Some(m) = start.find(text) {
        let first = text[m.start()..m.end()].trim_start();
        let pos = m.end() - first.len();
        positions.push((utf16_index(text, pos), utf16_index(text, m.end())));
    }
    if let Some(m) = end.find(text) {
        // The source pattern captures one mandatory preceding whitespace.
        let start = m.start() + text[m.start()..].chars().next().unwrap().len_utf8();
        let (first, last) = (utf16_index(text, start), utf16_index(text, m.end()));
        if !positions.iter().any(|(a, b)| first >= *a && first < *b) {
            positions.push((first, last))
        }
    }
    for m in verbose.find_iter(text) {
        positions.push((utf16_index(text, m.start()), utf16_index(text, m.end())))
    }
    positions
}
pub fn budget_continuation_message(pct: u32, used: u64, budget: u64) -> String {
    fn comma(n: u64) -> String {
        let s = n.to_string();
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',')
            }
            out.push(ch);
        }
        out
    }
    format!(
        "Stopped at {pct}% of token target ({} / {}). Keep working — do not summarize.",
        comma(used),
        comma(budget)
    )
}

// src/utils/plugins/pluginPolicy.ts
pub fn plugin_blocked_by_policy(id: &str, policy_enabled: &BTreeMap<String, bool>) -> bool {
    policy_enabled.get(id) == Some(&false)
}

// src/utils/suggestions/skillUsageTracking.ts
#[derive(Clone, Copy, Debug)]
pub struct SkillUsage {
    pub usage_count: u64,
    pub last_used_ms: i64,
}
pub fn skill_usage_score(usage: Option<SkillUsage>, now_ms: i64) -> f64 {
    let Some(usage) = usage else { return 0. };
    let days = (now_ms - usage.last_used_ms) as f64 / 86_400_000.;
    usage.usage_count as f64 * (0.5f64).powf(days / 7.).max(0.1)
}

// src/utils/systemDirectories.ts: safe directory ancestry predicate
pub fn is_inside_directory(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

// src/utils/agentSwarmsEnabled.ts: user-owned gate, without vendor tracking.
pub fn agent_teams_enabled(env_opt_in: bool, flag: bool, admin_allowed: bool) -> bool {
    (env_opt_in || flag) && admin_allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circular_buffer_eviction_and_clear() {
        let mut b = CircularBuffer::new(3).unwrap();
        b.add_all([1, 2, 3, 4]);
        assert_eq!(b.get_recent(2), vec![&3, &4]);
        assert_eq!(b.to_array(), vec![&2, &3, &4]);
        b.clear();
        b.add(5);
        assert_eq!(b.to_array(), vec![&5]);
    }
    #[test]
    fn original_array_placement_rules() {
        let mut v = vec![json!({"type":"tool_result"}), json!({"type":"text"})];
        insert_block_after_tool_results(&mut v, json!({"type":"cache_control"}));
        assert_eq!(v[1]["type"], "cache_control");
        let mut v = vec![json!({"type":"tool_result"})];
        insert_block_after_tool_results(&mut v, json!({"type":"cache_control"}));
        assert_eq!(v.last().unwrap()["text"], ".");
        let mut v = vec![json!({"type":"text"}), json!({"type":"image"})];
        insert_block_after_tool_results(&mut v, json!({"type":"cache_control"}));
        assert_eq!(v[1]["type"], "cache_control");
    }
    #[test]
    fn independent_utility_reference_cases() {
        assert_eq!(djb2_hash("a"), 97);
        assert_eq!(hash_pair("ab", "c").len(), 64);
        assert_ne!(hash_pair("ab", "c"), hash_pair("a", "bc"));
        assert_eq!(
            parse_peer_address("uds:/run/sock"),
            (PeerScheme::Uds, "/run/sock")
        );
        assert_eq!(
            parse_peer_address("bridge:session"),
            (PeerScheme::Bridge, "session")
        );
        assert_eq!(
            parse_peer_address("/run/sock"),
            (PeerScheme::Uds, "/run/sock")
        );
        assert_eq!(
            remove_sandbox_violation_tags(
                "before<sandbox_violations>bad\nline</sandbox_violations>after"
            ),
            "beforeafter"
        );
        assert_eq!(
            eager_parse_cli_flag("--x", &["--x=first".into(), "--x".into(), "second".into()]),
            Some("first".into())
        );
        assert_eq!(
            extract_args_after_double_dash("--", &["cmd".into(), "arg".into()]),
            ("cmd".into(), vec!["arg".into()])
        );
        assert_eq!(parse_token_budget("use 1.5M tokens"), Some(1_500_000.));
        assert_eq!(parse_token_budget("prefix +25k"), Some(25_000.));
        assert_eq!(find_token_budget_positions("+500k"), vec![(0, 5)]);
        assert_eq!(
            budget_continuation_message(35, 1200, 5000),
            "Stopped at 35% of token target (1,200 / 5,000). Keep working — do not summarize."
        );
        assert!(is_human_turn(&json!({"type":"user","content":"human"})));
        assert!(!is_human_turn(&json!({"type":"user","toolUseResult":{}})));
    }
    #[test]
    fn tags_paths_and_timeouts() {
        let tagged = to_tagged_id("user", "00000000-0000-0000-0000-000000000000").unwrap();
        assert_eq!(tagged, format!("user_01{}", "1".repeat(22)));
        assert!(to_tagged_id("x", "short").is_err());
        let x = XdgOptions {
            home: "/home/unit".into(),
            env: HashMap::new(),
        };
        assert_eq!(x.data_home(), PathBuf::from("/home/unit/.local/share"));
        let h = HashMap::from([("BASH_DEFAULT_TIMEOUT_MS".into(), "800000".into())]);
        assert_eq!(max_bash_timeout_ms(&h), 800_000);
        assert!(validate_uuid(&json!("01234567-89ab-cdef-0123-456789abcdef")).is_some());
        assert_eq!(macos_option_char("π"), Some("alt+p"));
        assert!(agent_teams_enabled(true, false, true));
    }
}
