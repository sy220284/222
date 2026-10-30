use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::process::Command;

fn run_pkce() -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .arg("oauth-pkce")
        .output()
        .expect("launch real OAuth PKCE CLI");
    assert!(
        result.status.success(),
        "OAuth PKCE CLI failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).expect("PKCE output is valid JSON")
}

#[test]
fn oauth_pkce_cli_uses_original_32_byte_crypto_and_independent_state() {
    let first = run_pkce();
    let second = run_pkce();
    for record in [&first, &second] {
        assert_eq!(record["challenge_method"], "S256");
        let verifier = record["verifier"].as_str().unwrap();
        let challenge = record["challenge"].as_str().unwrap();
        let state = record["state"].as_str().unwrap();
        for token in [verifier, challenge, state] {
            assert_eq!(token.len(), 43);
            assert!(token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
            assert_eq!(URL_SAFE_NO_PAD.decode(token).unwrap().len(), 32);
        }
        let calculated = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, calculated);
        assert_ne!(
            verifier, state,
            "verifier and anti-CSRF state must be independent"
        );
    }
    assert_ne!(first["verifier"], second["verifier"]);
    assert_ne!(first["state"], second["state"]);
}
