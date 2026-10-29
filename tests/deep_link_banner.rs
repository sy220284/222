use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

#[test]
fn deep_link_banner_reports_actual_checkout_origin_and_long_prefill() {
    let root = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["remote", "add", "origin", "git@github.com:team/repo.git"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let result = binary()
        .arg("--workspace")
        .arg(root.path())
        .args([
            "deep-link-banner",
            "--repo",
            "team/repo",
            "--prefill-length",
            "1200",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("external deep link"));
    assert!(text.contains("Resolved team/repo from local clones"));
    assert!(text.contains("last fetched never"));
    assert!(text.contains("AGENTS.md may be stale"));
    assert!(text.contains("1.2k chars"));
    assert!(text.contains("scroll to review"));

    let different = binary()
        .arg("--workspace")
        .arg(root.path())
        .args(["deep-link-banner", "--repo", "other/repo"])
        .output()
        .unwrap();
    assert!(different.status.success());
    let text = String::from_utf8(different.stdout).unwrap();
    assert!(text.contains("external deep link"));
    assert!(!text.contains("Resolved other/repo"));
    assert!(!text.contains("last fetched"));
}
