use anyhow::{bail, Context, Result};
use globset::{Glob, GlobMatcher};
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, time::SystemTime};
use walkdir::{DirEntry, WalkDir};

fn allowed(entry: &DirEntry) -> bool {
    if entry.depth() == 0 { return true }
    !matches!(entry.file_name().to_str(),
        Some(".git" | ".hg" | ".svn" | ".jj" | ".bzr" | ".sl" | ".atlas" | "target" | "node_modules"))
}
fn matcher(pattern: &str) -> Result<GlobMatcher> {
    if pattern.is_empty() { bail!("Glob pattern cannot be empty") }
    Ok(Glob::new(pattern)?.compile_matcher())
}

fn matching(matcher: &GlobMatcher, relative: &Path) -> bool {
    matcher.is_match(relative) ||
        relative.file_name().is_some_and(|n| matcher.is_match(Path::new(n)))
}
fn files(base: &Path) -> Result<Vec<PathBuf>> {
    if base.is_file() { return Ok(vec![base.to_path_buf()]) }
    let mut result = Vec::new();
    for item in WalkDir::new(base).follow_links(false).into_iter().filter_entry(allowed) {
        let item = item?;
        if item.file_type().is_file() { result.push(item.into_path()); }
    }
    Ok(result)
}
fn sort_newest(items: &mut [PathBuf]) {
    items.sort_by(|a,b| {
        let modified = |p: &Path| fs::metadata(p).ok()
            .and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
        modified(b).cmp(&modified(a)).then_with(|| a.cmp(b))
    });
}
fn relative<'a>(root: &'a Path, path: &'a Path) -> Result<&'a Path> {
    path.strip_prefix(root).context("Path not under workspace")
}
fn bounded_num(args: &Value, name: &str, default: u64, max: u64) -> Result<usize> {
    let number = match args.get(name) {
        Some(raw) => raw.as_u64().with_context(|| format!("{name} must be nonnegative integer"))?,
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
        "cpp" => matches!(extension, "cpp" | "cc" | "hpp" | "h"),
        other => extension == other,
    }
}

pub fn glob_files(root: &Path, base: &Path, args: &Value) -> Result<String> {
    if !base.is_dir() { bail!("Glob search path must be a directory") }
    let pattern = args.get("pattern").and_then(Value::as_str)
        .context("glob_files requires pattern")?;
    let compiled = matcher(pattern)?;
    let mut matches = files(base)?.into_iter()
        .filter(|p| p.strip_prefix(base).is_ok_and(|rel| matching(&compiled, rel)))
        .collect::<Vec<_>>();
    sort_newest(&mut matches);
    let truncated = matches.len() > 100;
    let paths = matches.iter().take(100)
        .map(|p| relative(root,p).map(|r| r.to_string_lossy().to_string()))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "filenames": paths,
        "numFiles": paths.len(),
        "truncated": truncated,
    }).to_string())
}

pub fn grep_files(root: &Path, base: &Path, args: &Value) -> Result<String> {
    if !base.exists() { bail!("Search path does not exist") }
    let pattern = args.get("pattern").and_then(Value::as_str)
        .context("grep_files requires pattern")?;
    let case_insensitive = args.get("-i").and_then(Value::as_bool).unwrap_or(false);
    let multi = args.get("multiline").and_then(Value::as_bool).unwrap_or(false);
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive).multi_line(multi)
        .dot_matches_new_line(multi).build()?;
    let file_pattern = args.get("glob").and_then(Value::as_str).map(matcher).transpose()?;
    let kind = args.get("type").and_then(Value::as_str);
    let mode = args.get("output_mode").and_then(Value::as_str).unwrap_or("files_with_matches");
    if !matches!(mode, "files_with_matches" | "count" | "content") {
        bail!("Invalid output_mode: {mode}")
    }
    let offset = bounded_num(args, "offset", 0, 100_000)?;
    let limit = bounded_num(args, "head_limit", 250, 10_000)?;
    let before = bounded_num(args, "-B", 0, 100)?
        .max(bounded_num(args, "-C", 0, 100)?)
        .max(bounded_num(args, "context", 0, 100)?);
    let after = bounded_num(args, "-A", 0, 100)?
        .max(bounded_num(args, "-C", 0, 100)?)
        .max(bounded_num(args, "context", 0, 100)?);
    let mut matches = Vec::new();
    let mut matched_files = 0usize;
    let mut matched_lines = 0usize;
    let mut candidates = files(base)?;
    sort_newest(&mut candidates);
    for path in candidates {
        if fs::metadata(&path)?.len() > 2_000_000 { continue }
        if let Some(kind) = kind {
            if !type_matches(&path, kind) { continue; }
        }
        let rel = path.strip_prefix(base).context("Invalid grep search root")?;
        if file_pattern.as_ref().is_some_and(|m| !matching(m,rel)) { continue }
        let Ok(content) = fs::read_to_string(&path) else { continue };
        let filename = relative(root,&path)?.to_string_lossy().to_string();
        if mode == "files_with_matches" {
            if regex.is_match(&content) {
                matched_files += 1;
                matches.push(filename);
            }
        } else if mode == "count" {
            let hits = regex.find_iter(&content).count();
            if hits > 0 {
                matched_files += 1;
                matched_lines += hits;
                matches.push(format!("{filename}:{hits}"));
            }
        } else {
            let lines: Vec<_> = content.lines().collect();
            let mut selected = vec![false;lines.len()];
            let mut matched = vec![false;lines.len()];
            for (index,line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    matched[index] = true;
                    for at in index.saturating_sub(before)..=(index+after).min(lines.len()-1) {
                        selected[at]=true;
                    }
                }
            }
            if matched.iter().any(|m| *m) {
                matched_files += 1;
                for (i,line) in lines.iter().enumerate() {
                    if !selected[i] { continue }
                    if matched[i] { matched_lines += 1; }
                    let marker = if matched[i] { ":" } else { "-" };
                    matches.push(format!("{filename}{marker}{}{marker}{line}",i+1));
                    if matches.len() > 20_000 { break; }
                }
            }
        }
        if matches.len() > 20_000 { break; }
    }
    let total = matches.len();
    let limit = if limit == 0 { 10_000 } else { limit };
    let page = matches.into_iter().skip(offset).take(limit).collect::<Vec<_>>();
    let truncated = total > offset.saturating_add(limit);
    Ok(json!({
        "mode":mode,
        "filenames":if mode == "files_with_matches" {page.clone()} else {vec![]},
        "content":page.join("\n"),
        "numFiles":matched_files,
        "numMatches":matched_lines,
        "appliedOffset":offset,
        "appliedLimit":if truncated {Some(limit)} else {None},
        "truncated":truncated,
    }).to_string())
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
                "-i":{"type":"boolean"},"-A":{"type":"integer"},"-B":{"type":"integer"},
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
        let first:Value = serde_json::from_str(&glob_files(root.path(),root.path(),
            &json!({"pattern":"**/*.rs"})).unwrap()).unwrap();
        assert_eq!(first["numFiles"],2);
        let counted:Value = serde_json::from_str(&grep_files(root.path(),root.path(),
            &json!({"pattern":"beta","-i":true,"type":"rust","output_mode":"count"}))
            .unwrap()).unwrap();
        assert_eq!(counted["numFiles"],2);
        assert_eq!(counted["numMatches"],3);
        let content:Value = serde_json::from_str(&grep_files(root.path(),root.path(),
            &json!({"pattern":"alpha","output_mode":"content","context":1})).unwrap()).unwrap();
        assert!(content["content"].as_str().unwrap().contains("one.rs:1:alpha"));
        assert!(content["content"].as_str().unwrap().contains("one.rs-2-Beta"));
        let paged:Value = serde_json::from_str(&grep_files(root.path(),root.path(),
            &json!({"pattern":"beta","-i":true,"output_mode":"files_with_matches",
                "head_limit":1,"offset":1})).unwrap()).unwrap();
        assert_eq!(paged["filenames"].as_array().unwrap().len(),1);
        assert!(grep_files(root.path(),root.path(),&json!({"pattern":"["})).is_err());
    }
}
