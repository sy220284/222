use std::process::Command;

#[test]
fn version_cli_prints_running_binary_version_and_optional_build_timestamp() {
    let binary = env!("CARGO_BIN_EXE_atlas-agent");
    let output = Command::new(binary).arg("version").output().unwrap();
    assert!(
        output.status.success(),
        "version command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual = String::from_utf8(output.stdout).unwrap();
    let expected = match option_env!("ATLAS_BUILD_TIME") {
        Some(timestamp) if !timestamp.is_empty() => {
            format!("{} (built {timestamp})", env!("CARGO_PKG_VERSION"))
        }
        _ => env!("CARGO_PKG_VERSION").to_owned(),
    };
    assert_eq!(actual.trim_end_matches(['\r', '\n']), expected);

    let clap_version = Command::new(binary).arg("--version").output().unwrap();
    assert!(
        clap_version.status.success(),
        "version flag failed: {}",
        String::from_utf8_lossy(&clap_version.stderr)
    );
    assert!(String::from_utf8_lossy(&clap_version.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn version_subcommand_does_not_load_malformed_user_settings() {
    let dir = tempfile::tempdir().unwrap();
    let bad_settings = dir.path().join("bad-user-settings.json");
    std::fs::write(&bad_settings, "{broken").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .arg("version")
        .env("ATLAS_USER_SETTINGS", &bad_settings)
        .env_remove("NODE_EXTRA_CA_CERTS")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "version command must not parse settings: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(
        output.trim().starts_with(env!("CARGO_PKG_VERSION")),
        "{output}"
    );
    assert!(result.stderr.is_empty());
}
