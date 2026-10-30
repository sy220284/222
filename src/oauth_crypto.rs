//! Provider-neutral OAuth PKCE (RFC 7636), ported from
//! src/services/oauth/crypto.ts in the 2.1.88 source.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};

fn secure_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| anyhow::anyhow!("System entropy unavailable: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn generate_code_verifier() -> anyhow::Result<String> {
    secure_token()
}

pub fn generate_code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn generate_state() -> anyhow::Result<String> {
    secure_token()
}

/// PKCE request state is generated together to avoid accidentally
/// pairing a new challenge with a previous verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceRequest {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl PkceRequest {
    pub fn create() -> anyhow::Result<Self> {
        let verifier = generate_code_verifier()?;
        let challenge = generate_code_challenge(&verifier);
        let state = generate_state()?;
        Ok(Self {
            verifier,
            challenge,
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_7636_appendix_b_challenge_matches_known_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            generate_code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn tokens_are_32_random_bytes_as_unpadded_base64url() {
        let first = PkceRequest::create().unwrap();
        let second = PkceRequest::create().unwrap();
        for value in [
            &first.verifier,
            &first.state,
            &second.verifier,
            &second.state,
        ] {
            assert_eq!(value.len(), 43);
            assert!(value
                .bytes()
                .all(|byte| { byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' }));
            assert_eq!(URL_SAFE_NO_PAD.decode(value).unwrap().len(), 32);
        }
        assert_ne!(first.verifier, second.verifier);
        assert_ne!(first.state, second.state);
        assert_ne!(first.verifier, first.state);
        assert_eq!(first.challenge, generate_code_challenge(&first.verifier));
        assert_eq!(first.challenge.len(), 43);
    }

    #[test]
    fn sha256_challenge_uses_utf8_verifier_bytes() {
        assert_eq!(
            generate_code_challenge("abc"),
            "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
        );
    }
}
