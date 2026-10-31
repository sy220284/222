use serde_json::json;
use std::{fs, process::Command};
#[test]
fn sessions_list_shows_branch_count_and_a_single_tag() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join(".atlas/sessions");
    fs::create_dir_all(&directory).unwrap();
    let id = uuid::Uuid::new_v4();
    fs::write(directory.join(format!("{id}.json")),json!({"id":id,"messages":[],"custom_title":"Tagged session","tag":"check","git_branch":"feature/test"}).to_string()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(), "sessions"])
        .env("ATLAS_CONFIG", root.path().join("config.json"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("Tagged session"));
    assert!(text.contains("feature/test"));
    assert!(text.contains("0 messages"));
    assert_eq!(text.matches("#check").count(), 1, "{text}");
}
