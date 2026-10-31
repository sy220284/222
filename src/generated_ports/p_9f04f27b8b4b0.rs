//! Native Rust port derived from original: src/utils/plugins/walkPluginMarkdown.ts
//!
//! Recursively walks a plugin directory, invoking the visitor for each `.md`
//! file. The namespace array tracks the subdirectory path relative to the
//! root (e.g. `["foo", "bar"]` for `root/foo/bar/file.md`). Callers that do
//! not need namespacing can ignore the second argument.
//!
//! When `stop_at_skill_dir` is true and a directory contains `SKILL.md`, the
//! visitor is called for all `.md` files in that directory but subdirectories
//! are not scanned — skill directories are leaf containers.
//!
//! Readdir errors are swallowed with a debug log so one bad directory does
//! not abort a plugin load.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use crate::generated_ports::p_5b257a1951fde::{log_for_debugging, DebugLogLevel};

fn is_skill_md(name: &str) -> bool {
    name.eq_ignore_ascii_case("skill.md")
}

fn is_markdown(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md")
}

fn read_dir_entries(dir_path: &Path) -> Option<Vec<(PathBuf, bool)>> {
    let entries = match fs::read_dir(dir_path) {
        Ok(entries) => entries,
        Err(error) => {
            log_for_debugging(
                &format!(
                    "Failed to scan plugin directory {}: {}",
                    dir_path.display(),
                    error
                ),
                DebugLogLevel::error,
            );
            return None;
        }
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(error) => {
                log_for_debugging(
                    &format!(
                        "Failed to stat {} while scanning plugin directory: {}",
                        path.display(),
                        error
                    ),
                    DebugLogLevel::error,
                );
                continue;
            }
        };
        out.push((path, file_type.is_file()));
    }
    Some(out)
}

fn scan(
    dir_path: &Path,
    namespace: &[String],
    stop_at_skill_dir: bool,
    log_label: &str,
    on_file: &mut dyn FnMut(&Path, &[String]),
) {
    let entries = match read_dir_entries(dir_path) {
        Some(entries) => entries,
        None => return,
    };

    if stop_at_skill_dir
        && entries
            .iter()
            .any(|(path, is_file)| *is_file && is_skill_md(&path_file_name(path)))
    {
        // Skill directory: collect .md files here, don't recurse.
        for (path, is_file) in &entries {
            if *is_file && is_markdown(&path_file_name(path)) {
                on_file(path, namespace);
            }
        }
        return;
    }

    for (path, is_file) in &entries {
        let name = path_file_name(path);
        if !is_file {
            let mut child = namespace.to_vec();
            child.push(name.clone());
            scan(path, &child, stop_at_skill_dir, log_label, on_file);
            continue;
        }
        if is_markdown(&name) {
            on_file(path, namespace);
        }
    }
}

fn path_file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub struct WalkPluginMarkdownOptions<'a> {
    pub stop_at_skill_dir: bool,
    pub log_label: &'a str,
}

impl<'a> Default for WalkPluginMarkdownOptions<'a> {
    fn default() -> Self {
        Self {
            stop_at_skill_dir: false,
            log_label: "plugin",
        }
    }
}

pub fn walk_plugin_markdown<F: FnMut(&Path, &[String])>(
    root_dir: &Path,
    options: Option<WalkPluginMarkdownOptions<'_>>,
    mut on_file: F,
) {
    let options = options.unwrap_or_default();
    scan(
        root_dir,
        &[],
        options.stop_at_skill_dir,
        options.log_label,
        &mut on_file,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(dir: &Path, rel: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"# doc").unwrap();
        path
    }

    #[test]
    fn walk_collects_markdown_with_namespaces() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "README.md");
        touch(root, "foo/bar/nested.md");
        touch(root, "foo/bar/ignore.txt");

        let mut found = Vec::new();
        walk_plugin_markdown(root, None, |path, ns| {
            found.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                ns.to_vec(),
            ));
        });
        let mut names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["README.md", "nested.md"]);
        let nested = found.iter().find(|(n, _)| n == "nested.md").unwrap();
        assert_eq!(nested.1, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn stop_at_skill_dir_treats_skill_directory_as_leaf() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "SKILL.md");
        touch(root, "extra.md");
        touch(root, "sub/should-not-visit.md");

        let mut found = Vec::new();
        walk_plugin_markdown(
            root,
            Some(WalkPluginMarkdownOptions {
                stop_at_skill_dir: true,
                ..Default::default()
            }),
            |path, ns| {
                found.push((
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    ns.to_vec(),
                ));
            },
        );
        let mut names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["SKILL.md", "extra.md"]);
        // Skill dir is root itself, so namespaces stay empty.
        assert!(found.iter().all(|(_, ns)| ns.is_empty()));
    }

    #[test]
    fn stop_at_skill_dir_ignores_case_of_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "skill.MD");
        touch(root, "here.md");

        let mut count = 0usize;
        walk_plugin_markdown(
            root,
            Some(WalkPluginMarkdownOptions {
                stop_at_skill_dir: true,
                ..Default::default()
            }),
            |_, _| count += 1,
        );
        assert_eq!(count, 2);
    }

    #[test]
    fn unreadable_directory_does_not_abort_walk() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "top.md");
        // A file where a directory is expected: read_dir errors, walk continues.
        let blocker = root.join("blocker.md");
        fs::write(&blocker, b"not a dir").unwrap();

        let mut found = Vec::new();
        walk_plugin_markdown(root, None, |path, _| {
            found.push(path.file_name().unwrap().to_string_lossy().into_owned());
        });
        assert_eq!(found.len(), 2);
    }
}
