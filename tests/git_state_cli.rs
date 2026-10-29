use serde_json::Value;
use std::{fs, process::Command};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

fn init(dir: &std::path::Path) {
    let result = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
}

#[test]
fn real_git_state_is_read_from_disk_and_remote_credentials_are_redacted() {
    let repo = tempfile::tempdir().unwrap();
    init(repo.path());
    let config = repo.path().join(".git/config");
    let existing = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        format!(
            "{existing}\n[remote \"origin\"]\nurl = https://user:SECRET@github.com/team/repo.git\n"
        ),
    )
    .unwrap();
    let output = binary()
        .arg("--workspace")
        .arg(repo.path())
        .arg("git-state")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let data: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["worktree_count"], 1);
    assert_eq!(data["remote"]["host"], "github.com");
    assert_eq!(data["remote"]["owner"], "team");
    assert_eq!(data["remote"]["name"], "repo");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("SECRET"));
}

#[test]
fn git_config_cli_parses_real_file_and_missing_keys() {
    let repo = tempfile::tempdir().unwrap();
    init(repo.path());
    let config = repo.path().join(".git/config");
    fs::write(
        &config,
        "[remote \"origin\"]\nurl = git@github.com:team/example.git\n[core]\nHooksPath = \"/tmp/custom hooks\"\n",
    )
    .unwrap();
    let output = binary()
        .arg("--workspace")
        .arg(repo.path())
        .args(["git-config", "remote", "url", "--subsection", "origin"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "git@github.com:team/example.git");
    let absent = binary()
        .arg("--workspace")
        .arg(repo.path())
        .args(["git-config", "core", "missing"])
        .output()
        .unwrap();
    assert!(absent.status.success());
    assert_eq!(String::from_utf8(absent.stdout).unwrap().trim(), "(not found)");
}

#[test]
fn git_ignore_cli_uses_real_nested_local_ignore_rules() {
    let repo = tempfile::tempdir().unwrap();
    init(repo.path());
    fs::create_dir(repo.path().join("nested")).unwrap();
    fs::write(repo.path().join("nested/.gitignore"), "*.secret\n").unwrap();
    for (candidate, expected) in [("nested/key.secret", "true"), ("nested/readme.md", "false")] {
        let result = binary()
            .arg("--workspace")
            .arg(repo.path())
            .args(["git-ignore", candidate])
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
    }
}
