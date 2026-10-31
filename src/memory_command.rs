//! Provider-neutral local port of commands/memory/memory.tsx and its selector.
//! Memory files are user-owned; this command never uploads or synchronizes data.
use anyhow::{bail, Context, Result};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryTarget {
    pub label: &'static str,
    pub path: PathBuf,
    pub create_if_missing: bool,
}

pub fn targets(workspace: &Path, user_config: &Path) -> Vec<MemoryTarget> {
    let candidates = [
        MemoryTarget {
            label: "User instructions",
            path: user_config.join("AGENTS.md"),
            create_if_missing: true,
        },
        MemoryTarget {
            label: "Project instructions",
            path: workspace.join("AGENTS.md"),
            create_if_missing: true,
        },
        MemoryTarget {
            label: "Auto-memory index",
            path: workspace.join(".atlas/memory/MEMORY.md"),
            create_if_missing: false,
        },
    ];
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|target| {
            // Only expose auto-memory after creation by the normal memory
            // engine; opening /memory must not fabricate an empty index.
            (target.create_if_missing || target.path.is_file()) && seen.insert(target.path.clone())
        })
        .collect()
}

pub fn select(targets: &[MemoryTarget], raw: &str) -> Result<Option<PathBuf>> {
    let answer = raw.trim();
    if answer.eq_ignore_ascii_case("q") || answer.eq_ignore_ascii_case("cancel") {
        return Ok(None);
    }
    let selected: usize = answer
        .parse()
        .context("Choose a listed memory-file number, or q to cancel")?;
    let target = targets
        .get(selected.saturating_sub(1))
        .filter(|_| selected > 0)
        .context("Memory-file selection out of range")?;
    Ok(Some(target.path.clone()))
}

/// Equivalent to original writeFile(flag='wx'): preexisting content is never
/// truncated, and every other filesystem error reaches the caller.
/// Refuse following symlinks in a memory-file path. The memory command
/// must not open or write arbitrary files outside the selected scope through
/// an existing link, including a dangling link.
fn verify_memory_path(path: &Path, scope: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(scope)
        .context("Memory file is outside its configured scope")?;
    let mut current = scope.to_path_buf();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(name) => current.push(name),
            _ => bail!("Memory path contains a non-normal component"),
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                bail!(
                    "Memory file path contains a symbolic link: {}",
                    current.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Inspect memory file path {}", current.display()))
            }
        }
    }
    Ok(())
}

pub fn ensure_exists(path: &Path) -> Result<()> {
    let directory = path
        .parent()
        .context("Memory file has no parent directory")?;
    fs::create_dir_all(directory)
        .with_context(|| format!("Create memory directory {}", directory.display()))?;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.flush()?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error).with_context(|| format!("Create memory file {}", path.display())),
    }
}

pub fn run(workspace: &Path) -> Result<String> {
    let config = crate::config::Config::path()?;
    let user_dir = config
        .parent()
        .context("User configuration has no parent directory")?;
    let candidates = targets(workspace, user_dir);
    println!("Memory files:");
    for (index, target) in candidates.iter().enumerate() {
        println!(
            "  {}) {}  {}{}",
            index + 1,
            target.label,
            target.path.display(),
            if target.path.exists() { "" } else { " (new)" },
        );
    }
    print!("Choose a memory file (q to cancel): ");
    io::stdout().flush()?;
    let mut choice = String::new();
    if io::stdin().read_line(&mut choice)? == 0 {
        return Ok("Cancelled memory editing".into());
    }
    let Some(chosen) = select(&candidates, &choice)? else {
        return Ok("Cancelled memory editing".into());
    };
    let target = candidates
        .iter()
        .find(|item| item.path == chosen)
        .context("Selected memory file disappeared from options")?;
    let scope = if target.label == "User instructions" {
        user_dir
    } else {
        workspace
    };
    verify_memory_path(&chosen, scope)?;
    if target.create_if_missing {
        ensure_exists(&chosen)?;
    }
    let (source, _launch) = crate::editor_runtime::open_file(&chosen, None)?;
    let hint = if source == "default" {
        "To use a different editor, set $EDITOR or $VISUAL.".into()
    } else {
        format!("Using {source}. To change editors, set $EDITOR or $VISUAL.")
    };
    Ok(format!(
        "Opened memory file at {}\n\n{hint}",
        chosen.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_runtime::parse_editor_command as editor_argv;

    #[test]
    fn selection_and_cancellation_never_create_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let options = targets(root, &root.join("user"));
        assert_eq!(options.len(), 2);
        assert_eq!(select(&options, "q").unwrap(), None);
        assert_eq!(select(&options, "cancel").unwrap(), None);
        assert!(select(&options, "0").is_err());
        assert!(select(&options, "99").is_err());
        assert!(select(&options, "bad").is_err());
        assert!(!root.join("AGENTS.md").exists());
        assert!(!root.join("user/AGENTS.md").exists());
    }

    #[test]
    fn create_new_preserves_existing_content_and_discovers_auto_memory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let project = root.join("AGENTS.md");
        ensure_exists(&project).unwrap();
        fs::write(&project, "important user text\n").unwrap();
        ensure_exists(&project).unwrap();
        assert_eq!(
            fs::read_to_string(&project).unwrap(),
            "important user text\n"
        );
        let auto = root.join(".atlas/memory/MEMORY.md");
        fs::create_dir_all(auto.parent().unwrap()).unwrap();
        fs::write(&auto, "facts").unwrap();
        let choices = targets(root, &root.join("user"));
        assert_eq!(choices.len(), 3);
        assert_eq!(select(&choices, "3").unwrap(), Some(auto));
    }

    #[cfg(unix)]
    #[test]
    fn memory_path_never_follows_links_even_when_the_target_is_missing() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = root.path().join(".atlas");
        symlink(outside.path(), &linked).unwrap();
        let path = linked.join("memory/MEMORY.md");
        assert!(verify_memory_path(&path, root.path()).is_err());
        let direct = root.path().join("AGENTS.md");
        symlink(outside.path().join("not-yet-created"), &direct).unwrap();
        assert!(verify_memory_path(&direct, root.path()).is_err());
        assert!(verify_memory_path(&root.path().join("clean.md"), root.path()).is_ok());
        assert!(verify_memory_path(&outside.path().join("other"), root.path()).is_err());
    }

    #[test]
    fn quoted_editor_argv_preserves_spaces_and_never_runs_a_shell() {
        assert_eq!(
            editor_argv(r#""/some path/code" --wait"#).unwrap(),
            vec!["/some path/code", "--wait"],
        );
        assert_eq!(
            editor_argv(r#"vi -u 'custom rc'"#).unwrap(),
            vec!["vi", "-u", "custom rc"],
        );
        assert!(editor_argv("'unterminated").is_err());
        assert!(editor_argv("").is_err());
    }
}
