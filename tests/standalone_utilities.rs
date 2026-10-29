use serde_json::Value;
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

#[test]
fn running_version_is_available_as_local_command() {
    let version = binary().arg("version").output().unwrap();
    assert!(
        version.status.success(),
        "{}",
        String::from_utf8_lossy(&version.stderr)
    );
    let text = String::from_utf8(version.stdout).unwrap();
    assert!(text.trim().starts_with(env!("CARGO_PKG_VERSION")));
}

#[test]
fn oauth_pkce_cli_uses_rfc_7636_sha256_with_independent_random_secrets() {
    let first = binary().arg("oauth-pkce").output().unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = binary().arg("oauth-pkce").output().unwrap();
    assert!(second.status.success());
    let a: Value = serde_json::from_slice(&first.stdout).unwrap();
    let b: Value = serde_json::from_slice(&second.stdout).unwrap();
    for item in [&a, &b] {
        for name in ["verifier", "challenge", "state"] {
            let token = item[name].as_str().unwrap();
            assert_eq!(token.len(), 43);
            assert!(token.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
            }));
        }
        assert_eq!(item["challenge_method"], "S256");
    }
    assert_ne!(a["verifier"], a["state"]);
    assert_ne!(a["verifier"], b["verifier"]);
    assert_ne!(a["state"], b["state"]);
}
