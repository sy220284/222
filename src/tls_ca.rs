//! User-approved additional trust roots for outbound HTTPS clients.
//! Original source: src/utils/caCerts.ts + caCertsConfig.ts.
//! Reqwest/rustls does not read Node's NODE_EXTRA_CA_CERTS automatically.
use anyhow::{bail, Context, Result};
use reqwest::{Certificate, Client, ClientBuilder};
use std::path::Path;

fn root_certificates(path: &Path) -> Result<Vec<Certificate>> {
    let contents = std::fs::read(path)
        .with_context(|| format!("Read additional CA certificates at {}", path.display()))?;
    let certs = Certificate::from_pem_bundle(&contents)
        .with_context(|| format!("Parse additional CA certificates at {}", path.display()))?;
    if certs.is_empty() {
        bail!(
            "Additional CA bundle has no certificates: {}",
            path.display()
        );
    }
    Ok(certs)
}

/// Extend rustls's default roots, retaining the platform/Mozilla trust store.
/// Invalid configuration must fail visibly, never disable server verification.
fn client_builder_for_path(file: Option<&Path>) -> Result<ClientBuilder> {
    let mut builder = Client::builder();
    if let Some(file) = file {
        for cert in root_certificates(file)? {
            builder = builder.add_root_certificate(cert);
        }
    }
    Ok(builder)
}

pub fn client_builder() -> Result<ClientBuilder> {
    let path = std::env::var_os("NODE_EXTRA_CA_CERTS").filter(|v| !v.is_empty());
    client_builder_for_path(path.as_deref().map(Path::new))
}

/// Only the explicit user's config may affect trust roots before the project
/// trust dialog. Project files, MCP server configs, and workspace data are not
/// consulted here.
pub fn apply_user_startup_config() -> Result<()> {
    let path = crate::config::Config::path()?;
    if !path.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Read user config {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(crate::json_read::strip_bom(&raw))
        .with_context(|| format!("Parse user config {}", path.display()))?;
    crate::generated_ports::p_8041a90f0b147::apply_extra_ca_certs_from_config(
        value.get("env"),
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_bundle_fails_instead_of_disabling_validation() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("invalid-ca.pem");
        std::fs::write(&file, b"not a PEM certificate").unwrap();
        assert!(root_certificates(&file).is_err());
        assert!(root_certificates(&dir.path().join("missing.pem")).is_err());
        assert!(client_builder_for_path(Some(&file)).is_err());
    }

    #[test]
    fn user_ca_certificate_bundle_is_trusted_without_replacing_default_roots() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("root-ca.pem");
        std::fs::write(&file, include_bytes!("../tests/extra-ca-test.pem")).unwrap();
        assert_eq!(root_certificates(&file).unwrap().len(), 1);
        assert!(client_builder_for_path(Some(&file))
            .unwrap()
            .build()
            .is_ok());
        assert!(client_builder_for_path(None).unwrap().build().is_ok());
    }
}
