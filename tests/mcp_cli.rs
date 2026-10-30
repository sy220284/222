//! Exercises the user-visible binary, scoped persistence and validation of
//! source commands/mcp/addCommand.ts without connecting any remote service.
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(workspace: &Path, config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .env("ATLAS_CONFIG", config)
        .arg("--workspace")
        .arg(workspace)
        .arg("mcp")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn command_line_add_list_remove_stdin_and_http() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("global.json");
    let output = run(
        root.path(),
        &config,
        &[
            "add",
            "--scope",
            "local",
            "-e",
            "API=abc=123",
            "local-tools",
            "--",
            "python",
            "-m",
            "tools",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let local_config = fs::read_dir(root.path().join("mcp-workspaces"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let local: Value = serde_json::from_str(&fs::read_to_string(&local_config).unwrap()).unwrap();
    let entry = &local["mcpServers"]["local-tools"];
    assert_eq!(entry["command"], "python");
    assert_eq!(entry["args"], serde_json::json!(["-m", "tools"]));
    assert_eq!(entry["env"]["API"], "abc=123");

    let output = run(
        root.path(),
        &config,
        &[
            "add",
            "-s",
            "project",
            "-t",
            "http",
            "-H",
            "Authorization: Bearer sample",
            "remote",
            "https://example.org/mcp",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let project: Value =
        serde_json::from_str(&fs::read_to_string(root.path().join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(
        project["mcpServers"]["remote"]["headers"]["Authorization"],
        "Bearer sample"
    );
    let list = run(root.path(), &config, &["list"]);
    assert!(list.status.success());
    let value: Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert!(value
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["name"] == "remote" && x["scope"] == "project"));

    let active = run(root.path(), &config, &["list", "--active"]);
    assert!(
        active.status.success(),
        "{}",
        String::from_utf8_lossy(&active.stderr)
    );
    let active: Value = serde_json::from_slice(&active.stdout).unwrap();
    assert_eq!(active.as_array().unwrap().len(), 2);

    let path = root.path().join(".mcp.json");
    let mut changed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    changed["mcpServers"]["remote"]["url"] = serde_json::json!("https://other.example.org/mcp");
    fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let active = run(root.path(), &config, &["list", "--active"]);
    let active: Value = serde_json::from_slice(&active.stdout).unwrap();
    assert_eq!(active.as_array().unwrap().len(), 1);
    assert_eq!(active[0]["name"], "local-tools");

    assert!(run(
        root.path(),
        &config,
        &["remove", "local-tools", "-s", "local"]
    )
    .status
    .success());
    assert!(
        run(root.path(), &config, &["remove", "remote", "-s", "project"])
            .status
            .success()
    );
    let list = run(root.path(), &config, &["list"]);
    assert!(serde_json::from_slice::<Value>(&list.stdout)
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let local: Value = serde_json::from_str(&fs::read_to_string(&local_config).unwrap()).unwrap();
    assert!(local["approvedProjectServers"]
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn user_scope_is_separate_and_secret_urls_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("user-config.json");
    let out = run(
        root.path(),
        &config,
        &[
            "add",
            "--scope",
            "user",
            "user-tools",
            "--",
            "python",
            "-m",
            "service",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let user: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(user["mcp_servers"][0]["id"], "user-tools");
    assert_eq!(
        user["mcp_servers"][0]["args"],
        serde_json::json!(["-m", "service"])
    );
    let bad = run(
        root.path(),
        &config,
        &[
            "add",
            "--scope",
            "user",
            "-t",
            "http",
            "bad",
            "https://user:password@example.org/mcp",
        ],
    );
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("without credentials"));
    let user: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(user["mcp_servers"].as_array().unwrap().len(), 1);
}
