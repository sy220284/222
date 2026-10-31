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

/// Only user-owned configuration is consulted before the project trust dialog.
/// Match 2.1.88 caCertsConfig.ts: user settings take priority over the
/// global config and an already-set process environment takes priority over
/// both. Project/local settings must never supply TLS trust roots here.
fn read_user_ca_env(path: &Path) -> Result<Option<serde_json::Value>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Read user CA settings at {}", path.display()))?;
    let settings: serde_json::Value = serde_json::from_str(crate::json_read::strip_bom(&raw))
        .with_context(|| format!("Parse user CA settings at {}", path.display()))?;
    Ok(settings.get("env").cloned())
}

fn user_settings_path() -> Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("ATLAS_USER_SETTINGS") {
        return Ok(path.into());
    }
    Ok(dirs::config_dir()
        .context("Cannot locate user settings directory")?
        .join("atlas-agent")
        .join("settings.json"))
}

fn startup_ca_from_paths(user_settings: &Path, global_config: &Path) -> Result<Option<String>> {
    let settings_env = read_user_ca_env(user_settings)?;
    let global_env = read_user_ca_env(global_config)?;
    Ok(
        crate::generated_ports::p_8041a90f0b147::get_extra_certs_path_from_config(
            settings_env.as_ref(),
            global_env.as_ref(),
        ),
    )
}

/// Read both independently managed user-owned configuration sources before
/// any network clients are constructed. No project-supplied data is loaded.
pub fn apply_user_startup_config() -> Result<()> {
    if std::env::var_os("NODE_EXTRA_CA_CERTS").is_some_and(|value| !value.is_empty()) {
        return Ok(());
    }
    let settings_path = user_settings_path()?;
    let global_path = crate::config::Config::path()?;
    if let Some(path) = startup_ca_from_paths(&settings_path, &global_path)? {
        std::env::set_var("NODE_EXTRA_CA_CERTS", path);
    }
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

    #[test]
    fn user_settings_override_global_config_without_reading_project_settings() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global-config.json");
        let user = dir.path().join("settings.json");
        std::fs::write(&global, r#"{"env":{"NODE_EXTRA_CA_CERTS":"/global.pem"}}"#).unwrap();
        std::fs::write(&user, r#"{"env":{"NODE_EXTRA_CA_CERTS":"/user.pem"}}"#).unwrap();
        assert_eq!(
            startup_ca_from_paths(&user, &global).unwrap().as_deref(),
            Some("/user.pem")
        );
        std::fs::remove_file(&user).unwrap();
        assert_eq!(
            startup_ca_from_paths(&user, &global).unwrap().as_deref(),
            Some("/global.pem")
        );
        std::fs::remove_file(&global).unwrap();
        assert_eq!(startup_ca_from_paths(&user, &global).unwrap(), None);
    }

    #[test]
    fn malformed_user_settings_report_path_without_using_global_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global.json");
        let user = dir.path().join("user.json");
        std::fs::write(&global, r#"{"env":{"NODE_EXTRA_CA_CERTS":"/global.pem"}}"#).unwrap();
        std::fs::write(&user, "{broken").unwrap();
        let error = startup_ca_from_paths(&user, &global).unwrap_err();
        assert!(error.to_string().contains("user.json"), "{error:#}");
    }
}
