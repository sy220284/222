use anyhow::{bail, Context, Result};
use globset::{Glob, GlobMatcher};
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use walkdir::{DirEntry, WalkDir};

fn allowed(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    !matches!(
        entry.file_name().to_str(),
        Some(
            ".git" | ".hg" | ".svn" | ".jj" | ".bzr" | ".sl" | ".atlas" | "target" | "node_modules"
        )
    )
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
/// such as "*.{ts,tsx}". Exclusion globs are applied after inclusions.
fn grep_globs(raw: Option<&str>) -> Result<(Vec<GlobMatcher>, Vec<GlobMatcher>)> {
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    for word in raw.unwrap_or("").split_whitespace() {
        let patterns: Vec<&str> = if word.contains('{') && word.contains('}') {
            vec![word]
        } else {
            word.split(',').filter(|s| !s.is_empty()).collect()
        };
        for pattern in patterns {
            if let Some(negative) = pattern.strip_prefix('!') {
                exclude.push(matcher(negative)?);
            } else {
                include.push(matcher(pattern)?);
            }
        }
    }
    Ok((include, exclude))
}

fn files(base: &Path) -> Result<Vec<PathBuf>> {
    if base.is_file() {
        return Ok(vec![base.to_path_buf()]);
    }
    let mut result = Vec::new();
    for item in WalkDir::new(base)
        .follow_links(false)
        .into_iter()
        .filter_entry(allowed)
    {
        let item = item?;
        if item.file_type().is_file() {
            result.push(item.into_path());
        }
    }
    Ok(result)
}
/// GrepTool uses ripgrep with gitignore enabled. Delegate nested/global
/// ignore precedence to Git rather than reimplementing it incompletely.
fn git_ignored_paths(base: &Path, paths: &[PathBuf]) -> std::collections::HashSet<PathBuf> {
    use std::process::{Command, Stdio};
    let mut ignored = std::collections::HashSet::new();
    let repo = Command::new("git")
        .arg("-C")
        .arg(base)
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdin(Stdio::null())
        .output();
    if !repo.is_ok_and(|output| output.status.success() && output.stdout.starts_with(b"true")) {
        return ignored;
    }
    for chunk in paths.chunks(128) {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(base)
            .args(["check-ignore", "-z", "--"])
            .stdin(Stdio::null());
        for path in chunk {
            if let Ok(relative) = path.strip_prefix(base) {
                cmd.arg(relative);
            }
        }
        let Ok(output) = cmd.output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        for name in output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            if let Ok(name) = std::str::from_utf8(name) {
                ignored.insert(base.join(name));
            }
        }
    }
    ignored
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
fn relative<'a>(root: &'a Path, path: &'a Path) -> Result<&'a Path> {
    Ok(path.strip_prefix(root).unwrap_or(path))
}
fn bounded_num(args: &Value, name: &str, default: u64, max: u64) -> Result<usize> {
    let number = match args.get(name) {
        Some(raw) => raw
            .as_u64()
            .with_context(|| format!("{name} must be nonnegative integer"))?,
        None => default,
    };
    Ok(number.min(max) as usize)
}
fn type_matches(path: &Path, kind: &str) -> bool {
    let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
    match kind {
        "rust" | "rs" => extension == "rs",
        "js" => matches!(extension, "js" | "jsx" | "mjs" | "cjs"),
        "ts" => matches!(extension, "ts" | "tsx"),
        "py" | "python" => extension == "py",
        "json" => matches!(extension, "json" | "jsonl"),
        "md" => matches!(extension, "md" | "mdx"),
        "go" => extension == "go",
        "java" => extension == "java",
        "cpp" | "c++" => matches!(
            extension,
            "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h"
        ),
        "c" => matches!(extension, "c" | "h"),
        "yaml" => matches!(extension, "yaml" | "yml"),
        "html" => matches!(extension, "html" | "htm"),
        "css" => matches!(extension, "css" | "scss" | "sass" | "less"),
        "sh" | "shell" => matches!(extension, "sh" | "bash" | "zsh" | "fish"),
        "toml" => extension == "toml",
        "xml" => matches!(extension, "xml" | "xsl" | "xslt" | "svg"),
        "web" => matches!(
            extension,
            "html" | "htm" | "css" | "scss" | "sass" | "less"
                | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx"
        ),
        other => extension == other,
    }
}

pub fn glob_files(root: &Path, base: &Path, args: &Value) -> Result<String> {
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
    let compiled = matcher(relative_pattern)?;
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
    let mut matches = files(base)?
        .into_iter()
        .filter(|p| {
            p.strip_prefix(base).is_ok_and(|rel| {
                matching(&compiled, rel)
                    && (include_hidden
                        || !rel
                            .components()
                            .any(|part| part.as_os_str().to_string_lossy().starts_with('.')))
            })
        })
        .collect::<Vec<_>>();
    if !skip_gitignore {
        let ignored = git_ignored_paths(base, &matches);
        matches.retain(|path| !ignored.contains(path));
    }
    sort_newest(&mut matches);
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
fn matching_lines(content: &str, regex: &regex::Regex, multiline: bool) -> Vec<bool> {
    let lines: Vec<&str> = content.lines().collect();
    let mut matched = vec![false; lines.len()];
    if !multiline {
        for (index, line) in lines.iter().enumerate() {
            matched[index] = regex.is_match(line);
        }
        return matched;
    }
    let starts: Vec<usize> = std::iter::once(0)
        .chain(content.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect();
    for found in regex.find_iter(content) {
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
    matched
}

pub fn grep_files(root: &Path, base: &Path, args: &Value) -> Result<String> {
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
    let (include_globs, exclude_globs) = grep_globs(args.get("glob").and_then(Value::as_str))?;
    let kind = args.get("type").and_then(Value::as_str);
    let mode = args
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("files_with_matches");
    if !matches!(mode, "files_with_matches" | "count" | "content") {
        bail!("Invalid output_mode: {mode}")
    }
    let offset = bounded_num(args, "offset", 0, 100_000)?;
    let requested_limit = bounded_num(args, "head_limit", 250, 100_000)?;
    // GrepTool.ts: explicit head_limit=0 disables pagination, not "show 10000".
    let limit = if requested_limit == 0 {
        usize::MAX
    } else {
        requested_limit
    };
    // The original -C/context option overrides -A and -B, rather than
    // taking their maximum. Explicit 0 must override other context flags.
    let around = if args.get("context").is_some() {
        Some(bounded_num(args, "context", 0, 100)?)
    } else if args.get("-C").is_some() {
        Some(bounded_num(args, "-C", 0, 100)?)
    } else {
        None
    };
    let (before, after) = if let Some(context) = around {
        (context, context)
    } else {
        (
            bounded_num(args, "-B", 0, 100)?,
            bounded_num(args, "-A", 0, 100)?,
        )
    };
    let show_line_numbers = args.get("-n").and_then(Value::as_bool).unwrap_or(true);
    let mut matches = Vec::new();
    let mut candidates = files(base)?;
    // GrepTool.ts dispatches ripgrep without --hidden. Match its default:
    // skip hidden descendants, but allow an explicitly selected hidden file.
    candidates.retain(|path| {
        if base.is_file() {
            return true;
        }
        path.strip_prefix(base).is_ok_and(|relative| {
            !relative.components().any(|part| {
                part.as_os_str()
                    .to_str()
                    .is_some_and(|name| name.starts_with('.'))
            })
        })
    });
    let ignored = git_ignored_paths(base, &candidates);
    candidates.retain(|path| !ignored.contains(path));
    sort_newest(&mut candidates);
    for path in candidates {
        if fs::metadata(&path)?.len() > 2_000_000 {
            continue;
        }
        if let Some(kind) = kind {
            if !type_matches(&path, kind) {
                continue;
            }
        }
        let rel = path
            .strip_prefix(base)
            .context("Invalid grep search root")?;
        if (!include_globs.is_empty() && !include_globs.iter().any(|m| matching(m, rel)))
            || exclude_globs.iter().any(|m| matching(m, rel))
        {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let filename = relative(root, &path)?.to_string_lossy().to_string();
        if mode == "files_with_matches" {
            // rg -l matches line-by-line unless -U is enabled. Checking the
            // whole file with a non-multiline regex incorrectly makes ^/$
            // anchors refer only to the beginning/end of the entire file.
            if matching_lines(&content, &regex, multiline)
                .into_iter()
                .any(|matched| matched)
            {
                matches.push(filename);
            }
            continue;
        }
        let lines: Vec<&str> = content.lines().collect();
        let matched = matching_lines(&content, &regex, multiline);
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
            if *is_match {
                for at in index.saturating_sub(before)..=(index + after).min(lines.len() - 1) {
                    selected[at] = true;
                }
            }
        }
        for (index, line) in lines.iter().enumerate() {
            if !selected[index] {
                continue;
            }
            let marker = if matched[index] { ":" } else { "-" };
            let location = if show_line_numbers {
                format!("{}{marker}", index + 1)
            } else {
                String::new()
            };
            matches.push(format!("{filename}{marker}{location}{line}"));
        }
    }
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
            "description":"Find up to 100 paths matching a wildcard glob; returns newest first and indicates truncation.",
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
