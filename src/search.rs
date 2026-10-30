use anyhow::{bail, Context, Result};
use globset::{Glob, GlobMatcher};
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};
use walkdir::{DirEntry, WalkDir};

// GrepTool.ts passes explicit VCS exclusions to ripgrep, whereas GlobTool
// delegates exclusions to its own glob/ignore options. Do not globally hide
// .atlas, target, node_modules or VCS files from both tools.
fn allowed(entry: &DirEntry, exclude_vcs: bool) -> bool {
    if !exclude_vcs || entry.depth() == 0 {
        return true;
    }
    !matches!(
        entry.file_name().to_str(),
        Some(".git" | ".hg" | ".svn" | ".jj" | ".bzr" | ".sl")
    )
}
/// The source orphanedPluginFilter.ts excludes a cached plugin version when
/// cache/<marketplace>/<plugin>/<version>/.orphaned_at exists. Both Glob and
/// Grep must hide those stale versions while keeping active versions visible.
/// Recognize the version-scoped marker only inside the plugin cache, so an
/// unrelated project file named .orphaned_at cannot hide user source files.
fn orphaned_plugin_cache_entry(path: &Path) -> bool {
    for version in path.ancestors() {
        let Some(plugin) = version.parent() else { continue };
        let Some(marketplace) = plugin.parent() else { continue };
        let Some(cache) = marketplace.parent() else { continue };
        if cache.file_name().and_then(|name| name.to_str()) != Some("cache") {
            continue;
        }
        let default_plugin_root = cache.parent().is_some_and(|plugins| {
            plugins.file_name().and_then(|name| name.to_str()) == Some("plugins")
                && plugins.parent().and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str()) == Some(".atlas")
        });
        let overridden_plugin_root = std::env::var_os("ATLAS_PLUGIN_CACHE_DIR")
            .is_some_and(|root| cache == Path::new(&root).join("cache").as_path());
        if !(default_plugin_root || overridden_plugin_root) {
            continue;
        }
        if fs::symlink_metadata(version.join(".orphaned_at"))
            .is_ok_and(|metadata| metadata.file_type().is_file())
        {
            return true;
        }
    }
    false
}

/// Original utils/glob.ts::extractGlobBaseDirectory searches an absolute
/// pattern from its static directory prefix, even when no path argument was
/// provided. The caller must authorize this directory before traversal.
pub fn absolute_glob_base(pattern: &str) -> Option<PathBuf> {
    let path = Path::new(pattern);
    if !path.is_absolute() {
        return None;
    }
    let wildcard = pattern.find(|ch| matches!(ch, '*' | '?' | '[' | '{'));
    match wildcard {
        Some(index) => {
            let prefix = &pattern[..index];
            // glob.ts::extractGlobBaseDirectory accepts both '/' and the
            // native Windows separator. A Windows absolute pattern may use
            // forward slashes even when MAIN_SEPARATOR is a backslash.
            let separator = prefix.rfind(|ch: char| {
                ch == std::path::MAIN_SEPARATOR || (cfg!(windows) && ch == '/')
            })?;
            Some(PathBuf::from(&prefix[..=separator]))
        }
        None => path.parent().map(Path::to_path_buf),
    }
}

fn matcher(pattern: &str) -> Result<GlobMatcher> {
    if pattern.is_empty() {
        bail!("Glob pattern cannot be empty")
    }
    Ok(Glob::new(pattern)?.compile_matcher())
}

fn matching(matcher: &GlobMatcher, relative: &Path) -> bool {
    matcher.is_match(relative)
        || relative
            .file_name()
            .is_some_and(|n| matcher.is_match(Path::new(n)))
}
/// GrepTool accepts space/comma-separated globs, preserving brace groups
/// such as "*.{ts,tsx}". Keep include/exclude precedence in input order.
/// GrepTool.ts passes --glob flags to ripgrep in the user's original
/// order. A later matching inclusion can override an earlier exclusion and
/// vice versa; splitting them into two unordered groups changes results.
fn grep_globs(raw: Option<&str>) -> Result<Vec<(bool, GlobMatcher)>> {
    let mut patterns = Vec::new();
    for word in raw.unwrap_or("").split_whitespace() {
        let entries: Vec<&str> = if word.contains('{') && word.contains('}') {
            vec![word]
        } else {
            word.split(',').filter(|part| !part.is_empty()).collect()
        };
        for pattern in entries {
            if let Some(negative) = pattern.strip_prefix('!') {
                patterns.push((false, matcher(negative)?));
            } else {
                patterns.push((true, matcher(pattern)?));
            }
        }
    }
    Ok(patterns)
}

fn grep_glob_allows(patterns: &[(bool, GlobMatcher)], path: &Path) -> bool {
    let mut allowed = !patterns.iter().any(|(include, _)| *include);
    for (include, pattern) in patterns {
        if matching(pattern, path) {
            allowed = *include;
        }
    }
    allowed
}

/// Explicit positive ripgrep --glob rules override .gitignore exclusions.
/// A default inclusion (no positive glob supplied) does not override them.
fn grep_glob_explicitly_includes(patterns: &[(bool, GlobMatcher)], path: &Path) -> bool {
    let mut decision = None;
    for (include, pattern) in patterns {
        if matching(pattern, path) {
            decision = Some(*include);
        }
    }
    decision == Some(true)
}

/// Prune denied directories before traversing them. Checking only the
/// returned files would still enter unreadable/denied subtrees and could
/// turn an otherwise valid search into an unexpected traversal error.
fn files_filtered<F: Fn(&Path) -> Result<bool>>(
    base: &Path,
    exclude_vcs: bool,
    allow_path: &F,
    deadline: Option<Instant>,
) -> Result<Vec<PathBuf>> {
    check_search_deadline(deadline)?;
    if orphaned_plugin_cache_entry(base) {
        return Ok(Vec::new());
    }
    if base.is_file() {
        return Ok(if allow_path(base)? {
            vec![base.to_path_buf()]
        } else {
            Vec::new()
        });
    }
    let mut result = Vec::new();
    let mut filter_error = None;
    for item in WalkDir::new(base)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !allowed(entry, exclude_vcs) || filter_error.is_some() {
                return false;
            }
            if let Err(error) = check_search_deadline(deadline) {
                filter_error = Some(error);
                return false;
            }
            if entry.depth() == 0 {
                return true;
            }
            if orphaned_plugin_cache_entry(entry.path()) {
                return false;
            }
            match allow_path(entry.path()) {
                Ok(allowed) => allowed,
                Err(error) => {
                    filter_error = Some(error);
                    false
                }
            }
        })
    {
        check_search_deadline(deadline)?;
        let item = item?;
        if item.file_type().is_file() {
            result.push(item.into_path());
        }
    }
    if let Some(error) = filter_error {
        return Err(error);
    }
    Ok(result)
}
/// GrepTool uses ripgrep with gitignore enabled. Delegate nested/global
/// ignore precedence to Git rather than reimplementing it incompletely.
/// Execute Git ignore probes under the same deadline as the parent Grep/Glob
/// search. The original ripgrep.ts terminates blocked search subprocesses;
/// an unbounded git check-ignore must not keep a timed-out search alive.
fn git_output_before_deadline(
    command: &mut std::process::Command,
    deadline: Instant,
) -> Result<std::process::Output> {
    use std::io::Read as _;
    use std::process::Stdio;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().context("Start Git ignore check")?;
    let mut stdout = child.stdout.take().context("Git stdout pipe unavailable")?;
    // Drain the pipe concurrently. Waiting for the child before reading can
    // deadlock when a large repository fills the operating system pipe buffer.
    let reader = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes)?;
        Ok(bytes)
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = reader.join()
                    .map_err(|_| anyhow::anyhow!("Git output reader panicked"))??;
                return Ok(std::process::Output {
                    status,
                    stdout: output,
                    stderr: Vec::new(),
                });
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                bail!("Git ignore check timed out; search results would be incomplete");
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(error).context("Wait for Git ignore check");
            }
        }
    }
}

/// Honor Git's nested ignore rules without silently returning unfiltered
/// results when the ignore subprocess fails or exceeds the search deadline.
fn git_ignored_paths(
    base: &Path,
    paths: &[PathBuf],
    deadline: Instant,
) -> Result<std::collections::HashSet<PathBuf>> {
    use std::process::Command;
    let mut ignored = std::collections::HashSet::new();
    if paths.is_empty() {
        return Ok(ignored);
    }
    check_search_deadline(Some(deadline))?;
    let mut probe = Command::new("git");
    probe.arg("-C").arg(base).args(["rev-parse", "--is-inside-work-tree"]);
    let repo = git_output_before_deadline(&mut probe, deadline)?;
    if !repo.status.success() || !repo.stdout.starts_with(b"true") {
        return Ok(ignored);
    }
    for chunk in paths.chunks(128) {
        check_search_deadline(Some(deadline))?;
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(base).args(["check-ignore", "-z", "--"]);
        let mut supplied = false;
        for path in chunk {
            if let Ok(relative) = path.strip_prefix(base) {
                cmd.arg(relative);
                supplied = true;
            }
        }
        if !supplied {
            continue;
        }
        let output = git_output_before_deadline(&mut cmd, deadline)?;
        // Git check-ignore returns 1 for zero matches; other failures must be
        // surfaced, otherwise ignored private paths may appear in the result.
        match output.status.code() {
            Some(0) | Some(1) => {}
            other => bail!("Git ignore check failed with exit status {other:?}"),
        }
        for name in output.stdout.split(|byte| *byte == 0).filter(|name| !name.is_empty()) {
            let relative = std::str::from_utf8(name)
                .context("Git ignore result contains invalid UTF-8")?;
            ignored.insert(base.join(relative));
        }
    }
    Ok(ignored)
}

/// ripgrep honors .ignore and .rgignore files in addition to Git's
/// .gitignore rules. The two ripgrep-specific sources have higher priority
/// than Git ignores; deeper directories override their ancestors.
struct RipgrepIgnoreRule {
    matcher: GlobMatcher,
    exclude: bool,
    directories_only: bool,
}

fn ripgrep_ignore_rules(
    directory: &Path,
    filename: &str,
) -> Result<Vec<RipgrepIgnoreRule>> {
    let path = directory.join(filename);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error)
            .with_context(|| format!("Read ripgrep ignore file {}", path.display())),
    };
    let mut rules = Vec::new();
    for (line_number, raw) in content.lines().enumerate() {
        let raw = raw.trim_end_matches('\r');
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let (exclude, raw) = if let Some(negated) = raw.strip_prefix('!') {
            (false, negated)
        } else {
            (true, raw)
        };
        let raw = raw.strip_prefix("\\#").or_else(|| raw.strip_prefix("\\!")).unwrap_or(raw);
        let directories_only = raw.ends_with('/');
        let raw = raw.trim_end_matches('/');
        if raw.is_empty() {
            continue;
        }
        let anchored = raw.starts_with('/');
        let raw = raw.trim_start_matches('/');
        let pattern = if !anchored && !raw.contains('/') {
            format!("**/{raw}")
        } else {
            raw.to_owned()
        };
        let matcher = Glob::new(&pattern)
            .with_context(|| format!("Invalid {} rule at {}:{}", filename, path.display(), line_number + 1))?
            .compile_matcher();
        rules.push(RipgrepIgnoreRule {
            matcher,
            exclude,
            directories_only,
        });
    }
    Ok(rules)
}

fn ripgrep_rule_matches(
    rule: &RipgrepIgnoreRule,
    relative: &Path,
    candidate_is_directory: bool,
) -> bool {
    let mut candidate = Some(relative);
    while let Some(path) = candidate {
        if !path.as_os_str().is_empty()
            && (!rule.directories_only || candidate_is_directory || path != relative)
            && rule.matcher.is_match(path)
        {
            return true;
        }
        candidate = path.parent();
    }
    false
}

/// Return an override only if a ripgrep-specific ignore rule actually
/// matches. A negated .ignore/.rgignore rule can re-include a Git-ignored
/// file; a positive rule hides files even outside a Git repository.
fn ripgrep_custom_ignore(
    base: &Path,
    paths: &[PathBuf],
    deadline: Instant,
) -> Result<std::collections::HashMap<PathBuf, bool>> {
    use std::collections::HashMap;
    let search_root = if base.is_dir() { base } else {
        base.parent().context("Search file has no parent directory")?
    };
    let mut cache: HashMap<(PathBuf, &'static str), Vec<RipgrepIgnoreRule>> = HashMap::new();
    let mut decisions = HashMap::new();
    for path in paths {
        check_search_deadline(Some(deadline))?;
        let parent = path.parent().context("Search candidate has no parent directory")?;
        let mut directories = Vec::new();
        let mut current = Some(parent);
        while let Some(directory) = current {
            if !directory.starts_with(search_root) {
                break;
            }
            directories.push(directory.to_path_buf());
            if directory == search_root {
                break;
            }
            current = directory.parent();
        }
        directories.reverse();
        // ripgrep prunes excluded directories before visiting their files.
        // A !child rule alone cannot resurrect a file inside a directory
        // excluded by parent/; the parent must first be re-included.
        let mut ignored_parent = false;
        let mut final_decision = None;
        for candidate in directories.iter().skip(1).chain(std::iter::once(path)) {
            check_search_deadline(Some(deadline))?;
            let is_directory = candidate != path;
            let mut decision = None;
            // The source priority is .rgignore > .ignore > .gitignore.
            // Within each source, the closest directory and last rule win.
            for filename in [".ignore", ".rgignore"] {
                let mut source_decision = None;
                for directory in &directories {
                    if directory == candidate || !candidate.starts_with(directory) {
                        continue;
                    }
                    let key = (directory.clone(), filename);
                    if !cache.contains_key(&key) {
                        cache.insert(key.clone(), ripgrep_ignore_rules(directory, filename)?);
                    }
                    let relative = candidate.strip_prefix(directory)
                        .context("Ignore rule directory does not contain candidate")?;
                    for rule in cache.get(&key).context("Ignore rule cache entry missing")? {
                        if ripgrep_rule_matches(rule, relative, is_directory) {
                            source_decision = Some(rule.exclude);
                        }
                    }
                }
                if source_decision.is_some() {
                    decision = source_decision;
                }
            }
            if is_directory && decision == Some(true) {
                ignored_parent = true;
                break;
            }
            final_decision = decision;
        }
        if ignored_parent {
            decisions.insert(path.clone(), true);
        } else if let Some(exclude) = final_decision {
            decisions.insert(path.clone(), exclude);
        }
    }
    Ok(decisions)
}

/// ripgrep.ts::ripGrep defaults to 20 seconds (60 on WSL) and reports
/// incomplete searches as errors. Do not return partial matches as success.
/// The provider-neutral override keeps the same positive-seconds semantics.
fn search_deadline() -> Result<Instant> {
    let default_seconds = if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        60
    } else {
        20
    };
    let seconds = std::env::var("ATLAS_GLOB_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(default_seconds);
    Instant::now()
        .checked_add(Duration::from_secs(seconds))
        .context("Search timeout exceeds supported range")
}

fn check_search_deadline(deadline: Option<Instant>) -> Result<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        bail!("Ripgrep search timed out before completing; results are incomplete")
    }
    Ok(())
}

fn sort_newest(items: &mut [PathBuf]) {
    items.sort_by(|a, b| {
        let modified = |p: &Path| {
            fs::metadata(p)
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        };
        modified(b).cmp(&modified(a)).then_with(|| a.cmp(b))
    });
}
// utils/glob.ts calls ripgrep --sort=modified (oldest first).
// GrepTool files_with_matches instead sorts newest first.
fn sort_oldest(items: &mut [PathBuf]) {
    items.sort_by(|a, b| {
        let modified = |p: &Path| {
            fs::metadata(p)
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        };
        modified(a).cmp(&modified(b)).then_with(|| a.cmp(b))
    });
}
fn relative<'a>(root: &'a Path, path: &'a Path) -> Result<&'a Path> {
    Ok(path.strip_prefix(root).unwrap_or(path))
}
fn nonnegative_num(args: &Value, name: &str, default: u64) -> Result<usize> {
    let number = match args.get(name) {
        Some(raw) => raw
            .as_u64()
            .with_context(|| format!("{name} must be nonnegative integer"))?,
        None => default,
    };
    // GrepTool.ts passes caller-supplied offsets, context sizes and limits
    // through to ripgrep. Silent caps returned fewer lines than requested.
    usize::try_from(number).with_context(|| format!("{name} exceeds platform index range"))
}
/// Validate original strict GlobTool/GrepTool inputs before permissions or
/// PreToolUse hooks. Schema-hidden semantic strings are normalized first,
/// so malformed requests cannot trigger read approval side effects.
pub fn validate_tool_input(name: &str, args: &Value) -> Result<()> {
    let mut normalized = args.clone();
    crate::semantic_inputs::normalize_tool_fields(name, &mut normalized)?;
    let object = normalized.as_object().context("Search arguments must be an object")?;
    let fields: &[&str] = match name {
        "glob_files" => &["pattern", "path"],
        "grep_files" => &[
            "pattern", "path", "glob", "output_mode", "-B", "-A", "-C",
            "context", "-n", "-i", "type", "head_limit", "offset", "multiline",
        ],
        _ => bail!("Unknown search tool: {name}"),
    };
    for (key, value) in object {
        if !fields.contains(&key.as_str()) {
            bail!("{name} unknown input field: {key}");
        }
        if matches!(key.as_str(), "pattern" | "path" | "glob" | "type" | "output_mode")
            && !value.is_string()
        {
            bail!("{name}.{key} must be a string");
        }
        if matches!(key.as_str(), "-i" | "-n" | "multiline") && !value.is_boolean() {
            bail!("{name}.{key} must be a boolean");
        }
        if matches!(
            key.as_str(), "-A" | "-B" | "-C" | "context" | "head_limit" | "offset"
        ) && value.as_u64().is_none()
        {
            bail!("{name}.{key} must be a nonnegative integer");
        }
    }
    if !object.get("pattern").is_some_and(Value::is_string) {
        bail!("{name} requires pattern");
    }
    if name == "grep_files" {
        if let Some(mode) = object.get("output_mode").and_then(Value::as_str) {
            if !matches!(mode, "content" | "files_with_matches" | "count") {
                bail!("Invalid output_mode: {mode}");
            }
        }
    }
    Ok(())
}


fn type_matches(path: &Path, kind: &str) -> Result<bool> {
    use std::{collections::HashMap, process::Command, sync::OnceLock};
    static TYPES: OnceLock<Result<HashMap<String, Vec<GlobMatcher>>, String>> = OnceLock::new();
    let types = TYPES.get_or_init(|| {
        // The original GrepTool delegates --type to ripgrep. Read the
        // installed ripgrep definitions once, including special filenames.
        let output = Command::new("rg").arg("--type-list").output()
            .map_err(|error| format!("Cannot load ripgrep file types: {error}"))?;
        if !output.status.success() {
            return Err(format!("ripgrep --type-list failed: {}",
                String::from_utf8_lossy(&output.stderr)));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|error| format!("Invalid ripgrep type list: {error}"))?;
        let mut types = HashMap::new();
        for entry in text.lines() {
            let (name, globs) = entry.split_once(": ")
                .ok_or_else(|| format!("Invalid ripgrep type entry: {entry}"))?;
            let mut matchers = Vec::new();
            for pattern in globs.split(", ") {
                let matcher = Glob::new(pattern)
                    .map_err(|error| format!("Invalid ripgrep type pattern for {name}: {error}"))?
                    .compile_matcher();
                matchers.push(matcher);
            }
            types.insert(name.to_owned(), matchers);
        }
        Ok(types)
    });
    let types = types.as_ref().map_err(|error| anyhow::anyhow!("{error}"))?;
    let alias = match kind {
        "rs" => "rust",
        "python" => "py",
        "shell" => "sh",
        "c++" => "cpp",
        other => other,
    };
    let matchers = types.get(alias)
        .with_context(|| format!("Unknown ripgrep file type: {kind}"))?;
    let name = path.file_name().context("Search file has no filename")?;
    Ok(matchers.iter().any(|matcher|
        matcher.is_match(path) || matcher.is_match(Path::new(name))))
}

pub fn glob_files(root: &Path, base: &Path, args: &Value) -> Result<String> {
    glob_files_filtered(root, base, args, &|_| Ok(true))
}

/// Apply the same per-file Read permission policy used by direct file tools.
/// A permitted search directory can contain explicitly denied descendants.
pub fn glob_files_filtered<F: Fn(&Path) -> Result<bool>>(
    root: &Path,
    base: &Path,
    args: &Value,
    allow_file: &F,
) -> Result<String> {
    validate_tool_input("glob_files", args)?;
    if !base.is_dir() {
        bail!("Glob search path must be a directory")
    }
    let pattern = args
        .get("pattern")
        .and_then(Value::as_str)
        .context("glob_files requires pattern")?;
    // The original Glob utility accepts absolute patterns. Resolve them
    // relative to the already permission-checked search root.
    let relative_pattern = if Path::new(pattern).is_absolute() {
        Path::new(pattern)
            .strip_prefix(base)
            .context("Absolute glob must be inside the approved search directory")?
            .to_str()
            .context("Glob pattern must be UTF-8")?
    } else {
        pattern
    };
    // ripgrep's glob syntax treats Windows directory separators as path
    // separators. globset treats a raw backslash as an escape, so normalize
    // the relative pattern before compiling it on Windows.
    let normalized_pattern = if cfg!(windows) {
        relative_pattern.replace('\\', "/")
    } else {
        relative_pattern.to_owned()
    };
    // utils/glob.ts passes the user's pattern as a ripgrep --glob argument.
    // A leading ! is a negative glob: discover all files except its matches.
    let (exclude_pattern, effective_pattern) = match normalized_pattern.strip_prefix('!') {
        Some(negative) => (true, negative),
        None => (false, normalized_pattern.as_str()),
    };
    let compiled = matcher(effective_pattern)?;
    let hidden = std::env::var("ATLAS_GLOB_HIDDEN")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "true".to_owned());
    let include_hidden = matches!(
        hidden.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    );
    let no_ignore = std::env::var("ATLAS_GLOB_NO_IGNORE")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "true".to_owned());
    let skip_gitignore = matches!(
        no_ignore.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    );
    // utils/glob.ts delegates discovery to ripGrep and inherits its timeout.
    // A timed-out traversal must be an error, never a shortened successful list.
    let deadline = search_deadline()?;
    let mut matches = files_filtered(base, false, allow_file, Some(deadline))?
        .into_iter()
        .filter(|p| {
            p.strip_prefix(base).is_ok_and(|rel| {
                (matching(&compiled, rel) != exclude_pattern)
                    && (include_hidden
                        || !rel
                            .components()
                            .any(|part| part.as_os_str().to_string_lossy().starts_with('.')))
            })
        })
        .collect::<Vec<_>>();
    check_search_deadline(Some(deadline))?;
    if !skip_gitignore {
        let ignored = git_ignored_paths(base, &matches, deadline)?;
        let custom = ripgrep_custom_ignore(base, &matches, deadline)?;
        check_search_deadline(Some(deadline))?;
        matches.retain(|path| custom.get(path).copied().unwrap_or_else(|| ignored.contains(path)) == false);
    }
    let mut visible = Vec::with_capacity(matches.len());
    for path in matches {
        check_search_deadline(Some(deadline))?;
        if allow_file(&path)? {
            visible.push(path);
        }
    }
    let mut matches = visible;
    sort_oldest(&mut matches);
    check_search_deadline(Some(deadline))?;
    let truncated = matches.len() > 100;
    let paths = matches
        .iter()
        .take(100)
        .map(|p| relative(root, p).map(|r| r.to_string_lossy().to_string()))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "filenames": paths,
        "numFiles": paths.len(),
        "truncated": truncated,
    })
    .to_string())
}

/// Return the line numbers covered by a regex match. In multiline mode a
/// single match can cover several lines, all of which ripgrep displays.
/// Search large files cooperatively under the same deadline as directory
/// traversal. The source ripgrep process can be killed on timeout; this Rust
/// matcher must check its budget while processing lines and multiline hits.
fn matching_lines(
    content: &str,
    regex: &regex::Regex,
    multiline: bool,
    deadline: Instant,
) -> Result<Vec<bool>> {
    let lines: Vec<&str> = content.lines().collect();
    let mut matched = vec![false; lines.len()];
    if !multiline {
        for (index, line) in lines.iter().enumerate() {
            if index % 256 == 0 {
                check_search_deadline(Some(deadline))?;
            }
            matched[index] = regex.is_match(line);
        }
        check_search_deadline(Some(deadline))?;
        return Ok(matched);
    }
    let starts: Vec<usize> = std::iter::once(0)
        .chain(content.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect();
    for (match_index, found) in regex.find_iter(content).enumerate() {
        if match_index % 256 == 0 {
            check_search_deadline(Some(deadline))?;
        }
        if lines.is_empty() {
            break;
        }
        let first = starts
            .partition_point(|&offset| offset <= found.start())
            .saturating_sub(1);
        let last_byte = found.end().saturating_sub(1).max(found.start());
        let last = starts
            .partition_point(|&offset| offset <= last_byte)
            .saturating_sub(1);
        for index in first..=last.min(lines.len() - 1) {
            matched[index] = true;
        }
    }
    check_search_deadline(Some(deadline))?;
    Ok(matched)
}

pub fn grep_files(root: &Path, base: &Path, args: &Value) -> Result<String> {
    grep_files_filtered(root, base, args, &|_| Ok(true))
}

/// Filter every candidate against the caller's Read rules before opening it.
/// This also prevents file-name and count modes from exposing denied paths.
pub fn grep_files_filtered<F: Fn(&Path) -> Result<bool>>(
    root: &Path,
    base: &Path,
    args: &Value,
    allow_file: &F,
) -> Result<String> {
    validate_tool_input("grep_files", args)?;
    if !base.exists() {
        bail!("Search path does not exist")
    }
    let pattern = args
        .get("pattern")
        .and_then(Value::as_str)
        .context("grep_files requires pattern")?;
    let case_insensitive = args.get("-i").and_then(Value::as_bool).unwrap_or(false);
    let multiline = args
        .get("multiline")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline)
        .build()?;
    let glob_patterns = grep_globs(args.get("glob").and_then(Value::as_str))?;
    let kind = args.get("type").and_then(Value::as_str);
    let mode = args
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("files_with_matches");
    if !matches!(mode, "files_with_matches" | "count" | "content") {
        bail!("Invalid output_mode: {mode}")
    }
    let offset = nonnegative_num(args, "offset", 0)?;
    let requested_limit = nonnegative_num(args, "head_limit", 250)?;
    // GrepTool.ts: explicit head_limit=0 disables pagination, not "show 10000".
    let limit = if requested_limit == 0 {
        usize::MAX
    } else {
        requested_limit
    };
    // The original -C/context option overrides -A and -B, rather than
    // taking their maximum. Explicit 0 must override other context flags.
    let around = if args.get("context").is_some() {
        Some(nonnegative_num(args, "context", 0)?)
    } else if args.get("-C").is_some() {
        Some(nonnegative_num(args, "-C", 0)?)
    } else {
        None
    };
    let (before, after) = if let Some(context) = around {
        (context, context)
    } else {
        (
            nonnegative_num(args, "-B", 0)?,
            nonnegative_num(args, "-A", 0)?,
        )
    };
    let show_line_numbers = args.get("-n").and_then(Value::as_bool).unwrap_or(true);
    let mut matches = Vec::new();
    // ripgrep separates disjoint context groups (including adjacent files).
    // The separators are part of its returned lines and count toward pagination.
    let mut emitted_context_file = false;
    // GrepTool.ts explicitly passes --hidden. Hidden project files and
    // directories must be searchable; only the six VCS directories above
    // are excluded. GlobTool uses a separate --hidden configuration.
    let deadline = search_deadline()?;
    let mut candidates = files_filtered(base, true, allow_file, Some(deadline))?;
    check_search_deadline(Some(deadline))?;
    let ignored = git_ignored_paths(base, &candidates, deadline)?;
    let custom = ripgrep_custom_ignore(base, &candidates, deadline)?;
    check_search_deadline(Some(deadline))?;
    candidates.retain(|path| {
        let is_ignored = custom.get(path).copied().unwrap_or_else(|| ignored.contains(path));
        if !is_ignored {
            return true;
        }
        path.strip_prefix(base).is_ok_and(|relative| {
            grep_glob_explicitly_includes(&glob_patterns, relative)
        })
    });
    // GrepTool.ts::call sorts by mtime only in files_with_matches mode.
    // Content and count modes preserve ripgrep's discovery order so offset
    // pagination does not unexpectedly reshuffle results by file age.
    if mode == "files_with_matches" {
        sort_newest(&mut candidates);
    }
    for path in candidates {
        check_search_deadline(Some(deadline))?;
        // Source GrepTool.ts applies --max-columns=500 to output lines,
        // not a whole-file size cap. Large log/source files still need
        // to be searched for matches.
        if let Some(kind) = kind {
            if !type_matches(&path, kind)? {
                continue;
            }
        }
        let rel = path
            .strip_prefix(base)
            .context("Invalid grep search root")?;
        if !grep_glob_allows(&glob_patterns, rel) {
            continue;
        }
        if !allow_file(&path)? {
            continue;
        }
        // ripgrep searches valid text around malformed UTF-8 bytes. Rejecting
        // the entire file with read_to_string silently loses those matches.
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            // A file removed between traversal and reading should not abort
            // an otherwise useful search. Permission and I/O failures must
            // remain visible instead of pretending the search was complete.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("Reading search file {}", path.display()));
            }
        };
        let binary = bytes.contains(&0);
        // When scanning a directory, ripgrep suppresses binary file results.
        // An explicitly requested file remains searchable in -l/-c modes.
        if binary && base.is_dir() {
            continue;
        }
        check_search_deadline(Some(deadline))?;
        let content = String::from_utf8_lossy(&bytes);
        let filename = relative(root, &path)?.to_string_lossy().to_string();
        if binary && mode == "content" {
            if matching_lines(&content, &regex, multiline, deadline)?
                .into_iter()
                .any(|matched| matched)
            {
                let nul_offset = bytes.iter().position(|byte| *byte == 0).unwrap_or(0);
                matches.push(format!(
                    "binary file matches (found \"\\0\" byte around offset {nul_offset})"
                ));
            }
            continue;
        }
        if mode == "files_with_matches" {
            // rg -l matches line-by-line unless -U is enabled. Checking the
            // whole file with a non-multiline regex incorrectly makes ^/$
            // anchors refer only to the beginning/end of the entire file.
            if matching_lines(&content, &regex, multiline, deadline)?
                .into_iter()
                .any(|matched| matched)
            {
                matches.push(filename);
            }
            continue;
        }
        let lines: Vec<&str> = content.lines().collect();
        let matched = matching_lines(&content, &regex, multiline, deadline)?;
        if mode == "count" {
            // ripgrep -c counts matching lines, not every occurrence on a line.
            let hits = matched.iter().filter(|m| **m).count();
            if hits > 0 {
                matches.push(format!("{filename}:{hits}"));
            }
            continue;
        }
        if !matched.iter().any(|m| *m) {
            continue;
        }
        let mut selected = vec![false; lines.len()];
        for (index, is_match) in matched.iter().enumerate() {
            if index % 256 == 0 {
                check_search_deadline(Some(deadline))?;
            }
            if *is_match {
                for at in index.saturating_sub(before)..=index.saturating_add(after).min(lines.len() - 1) {
                    selected[at] = true;
                }
            }
        }
        let context_enabled = before > 0 || after > 0;
        if context_enabled && emitted_context_file {
            matches.push("--".to_owned());
        }
        // rg --max-columns=500 measures bytes, including the line ending.
        // split_inclusive preserves LF/CRLF byte lengths while lines() strips them.
        let line_byte_lengths = bytes
            .split_inclusive(|byte| *byte == b'\n')
            .map(<[u8]>::len)
            .collect::<Vec<_>>();
        let mut previous_selected = None;
        for (index, line) in lines.iter().enumerate() {
            if index % 256 == 0 {
                check_search_deadline(Some(deadline))?;
            }
            if !selected[index] {
                continue;
            }
            if context_enabled && previous_selected.is_some_and(|last| index > last + 1) {
                matches.push("--".to_owned());
            }
            let is_match = matched[index];
            let marker = if is_match { ":" } else { "-" };
            let location = if show_line_numbers {
                format!("{}{marker}", index + 1)
            } else {
                String::new()
            };
            let displayed_line = if line_byte_lengths
                .get(index)
                .copied()
                .unwrap_or(line.len()) > 500
            {
                if is_match {
                    "[Omitted long matching line]"
                } else {
                    "[Omitted long context line]"
                }
            } else {
                line
            };
            matches.push(format!("{filename}{marker}{location}{displayed_line}"));
            previous_selected = Some(index);
        }
        emitted_context_file = context_enabled;
    }
    check_search_deadline(Some(deadline))?;
    let total = matches.len();
    let page = matches
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let truncated = total > offset.saturating_add(limit);
    let num_files = if mode == "files_with_matches" {
        page.len()
    } else if mode == "count" {
        page.len()
    } else {
        0
    };
    let num_matches = if mode == "count" {
        page.iter()
            .filter_map(|line| {
                line.rsplit_once(':')
                    .and_then(|(_, n)| n.parse::<usize>().ok())
            })
            .sum()
    } else {
        0
    };
    Ok(json!({
        "mode":mode,
        "filenames":if mode == "files_with_matches" {page.clone()} else {vec![]},
        "content":page.join("\n"),
        "numFiles":num_files,
        "numLines":if mode == "content" {Some(page.len())} else {None},
        "numMatches":num_matches,
        "appliedOffset":offset,
        "appliedLimit":if truncated {Some(requested_limit)} else {None},
        "truncated":truncated,
    })
    .to_string())
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"glob_files",
            "description":"Find up to 100 paths matching a wildcard glob; returns oldest first and indicates truncation.",
            "parameters":{"type":"object","properties":{
                "pattern":{"type":"string"},"path":{"type":"string"}
            },"required":["pattern"]}}}),
        json!({"type":"function","function":{"name":"grep_files",
            "description":"Search regex in files. output_mode: content, files_with_matches or count; supports glob, type, case-insensitivity, context, offset and head_limit.",
            "parameters":{"type":"object","properties":{
                "pattern":{"type":"string"},"path":{"type":"string"},"glob":{"type":"string"},
                "type":{"type":"string"},"output_mode":{"type":"string","enum":["content","files_with_matches","count"]},
                "-i":{"type":"boolean"},"-n":{"type":"boolean"},"-A":{"type":"integer"},"-B":{"type":"integer"},
                "-C":{"type":"integer"},"context":{"type":"integer"},
                "head_limit":{"type":"integer"},"offset":{"type":"integer"},
                "multiline":{"type":"boolean"}
            },"required":["pattern"]}}}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grep_uses_ripgrep_file_types_for_special_file_names() {
        assert!(type_matches(Path::new("project/Makefile"), "make").unwrap());
        assert!(type_matches(Path::new("project/CMakeLists.txt"), "cmake").unwrap());
        assert!(type_matches(Path::new("project/Dockerfile"), "docker").unwrap());
        assert!(type_matches(Path::new("project/main.rs"), "rust").unwrap());
        assert!(!type_matches(Path::new("project/main.ts"), "rust").unwrap());
        assert!(type_matches(Path::new("project/main.foo"), "invalid_type_222").is_err());
    }
    #[test]
    fn grep_respects_nested_ripgrep_ignore_files_and_override_priority() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sub")).unwrap();
        fs::write(root.path().join(".ignore"), "*.tmp\n!allowed.tmp\n").unwrap();
        fs::write(root.path().join("one.tmp"), "needle").unwrap();
        fs::write(root.path().join("allowed.tmp"), "needle").unwrap();
        fs::write(root.path().join("special.tmp"), "needle").unwrap();
        fs::write(root.path().join("sub/.rgignore"), "*.txt\n").unwrap();
        fs::write(root.path().join("sub/hidden.txt"), "needle").unwrap();
        fs::write(root.path().join("sub/visible.rs"), "needle").unwrap();
        fs::create_dir(root.path().join("blocked")).unwrap();
        fs::write(root.path().join("blocked/keep.rs"), "needle").unwrap();
        fs::write(root.path().join(".rgignore"), "!special.tmp\nblocked/\n!blocked/keep.rs\n").unwrap();
        let result: Value = serde_json::from_str(
            &grep_files(root.path(), root.path(),
                &json!({"pattern":"needle","output_mode":"files_with_matches"})).unwrap()
        ).unwrap();
        let files = result["filenames"].as_array().unwrap();
        let names: Vec<&str> = files.iter().filter_map(Value::as_str).collect();
        assert!(names.iter().any(|name| name.ends_with("allowed.tmp")));
        assert!(names.iter().any(|name| name.ends_with("special.tmp")));
        assert!(names.iter().any(|name| name.ends_with("visible.rs")));
        assert!(!names.iter().any(|name| name.ends_with("one.tmp")));
        assert!(!names.iter().any(|name| name.ends_with("hidden.txt")));
        assert!(!names.iter().any(|name| name.ends_with("blocked/keep.rs")));
        // A user-supplied positive glob overrides an ignore exclusion.
        let included: Value = serde_json::from_str(
            &grep_files(root.path(), root.path(),
                &json!({"pattern":"needle","glob":"one.tmp",
                    "output_mode":"files_with_matches"})).unwrap()
        ).unwrap();
        assert!(included["filenames"].as_array().unwrap().iter()
            .filter_map(Value::as_str).any(|name| name.ends_with("one.tmp")));
    }
    #[test]
    fn glob_and_grep_modes_pagination_and_filters() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sub")).unwrap();
        fs::write(root.path().join("one.rs"), "alpha\nBeta\nthird\n").unwrap();
        fs::write(root.path().join("sub/two.rs"), "beta\nbeta\n").unwrap();
        fs::write(root.path().join("sub/no.txt"), "beta").unwrap();
        let first: Value = serde_json::from_str(
            &glob_files(root.path(), root.path(), &json!({"pattern":"**/*.rs"})).unwrap(),
        )
        .unwrap();
        assert_eq!(first["numFiles"], 2);
        let counted: Value = serde_json::from_str(
            &grep_files(
                root.path(),
                root.path(),
                &json!({"pattern":"beta","-i":true,"type":"rust","output_mode":"count"}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(counted["numFiles"], 2);
        assert_eq!(counted["numMatches"], 3);
        let content: Value = serde_json::from_str(
            &grep_files(
                root.path(),
                root.path(),
                &json!({"pattern":"alpha","output_mode":"content","context":1}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(content["content"]
            .as_str()
            .unwrap()
            .contains("one.rs:1:alpha"));
        assert!(content["content"]
            .as_str()
            .unwrap()
            .contains("one.rs-2-Beta"));
        let paged: Value = serde_json::from_str(
            &grep_files(
                root.path(),
                root.path(),
                &json!({"pattern":"beta","-i":true,"output_mode":"files_with_matches",
                "head_limit":1,"offset":1}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(paged["filenames"].as_array().unwrap().len(), 1);
        assert!(grep_files(root.path(), root.path(), &json!({"pattern":"["})).is_err());
    }
}
