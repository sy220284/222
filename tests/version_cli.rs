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
    assert!(
        String::from_utf8_lossy(&clap_version.stdout).contains(env!("CARGO_PKG_VERSION"))
    );
}
