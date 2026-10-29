//! Shared provider-neutral environment parsing based on the 2.1.88 envUtils
//! semantics. No vendor-specific keychain access or protected-cluster telemetry.
use anyhow::{bail, Context, Result};
use std::{collections::HashMap, path::PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Match the original exact set of supported truth values, ignoring case and
/// surrounding whitespace; other non-empty strings are deliberately false.
pub fn is_env_truthy(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// An unset or blank variable is different from an explicitly false variable.
pub fn is_env_defined_falsy(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

/// Parse KEY=VALUE flags, retaining all subsequent '=' characters verbatim.
/// Repeated keys are overwritten by the final assignment, like the source.
pub fn parse_env_vars(raw: &[String]) -> Result<HashMap<String, String>> {
    let mut values = HashMap::new();
    for item in raw {
        let Some((key, value)) = item.split_once('=') else {
            bail!("Invalid environment variable format: {item}, environment variables should be added as: -e KEY1=value1 -e KEY2=value2");
        };
        if key.is_empty() {
            bail!("Invalid environment variable format: {item}, environment variables should be added as: -e KEY1=value1 -e KEY2=value2");
        }
        values.insert(key.to_owned(), value.to_owned());
    }
    Ok(values)
}

/// Replacement for --bare / SIMPLE mode; all keychain/credential fetches are
/// disabled by the caller, while explicit model keys may still be supplied.
pub fn bare_mode(env: Option<&str>, argv: &[String]) -> bool {
    is_env_truthy(env) || argv.iter().any(|arg| arg == "--bare")
}

/// Check an exact whitespace-delimited option rather than substring matches.
pub fn has_process_option(options: Option<&str>, flag: &str) -> bool {
    options.is_some_and(|value| value.split_whitespace().any(|token| token == flag))
}

/// Config home moves to the provider-neutral Atlas location. A value of ''
/// is considered an explicit override (the original uses ??, not ||).
pub fn config_home_dir(override_path: Option<&str>) -> Result<PathBuf> {
    let path = match override_path {
        Some(raw) => PathBuf::from(raw),
        None => dirs::home_dir()
            .context("Home directory unavailable")?
            .join(".atlas"),
    };
    Ok(PathBuf::from(
        path.to_string_lossy().nfc().collect::<String>(),
    ))
}

pub fn teams_dir(override_path: Option<&str>) -> Result<PathBuf> {
    Ok(config_home_dir(override_path)?.join("teams"))
}

pub fn aws_region<'a>(primary: Option<&'a str>, alternate: Option<&'a str>) -> &'a str {
    primary
        .filter(|v| !v.is_empty())
        .or_else(|| alternate.filter(|v| !v.is_empty()))
        .unwrap_or("us-east-1")
}

pub fn default_vertex_region(region: Option<&str>) -> &str {
    region.filter(|v| !v.is_empty()).unwrap_or("us-east5")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truthy_and_explicit_false_exact_source_matrix() {
        for v in ["1", "TRUE", " Yes ", "ON"] {
            assert!(is_env_truthy(Some(v)));
            assert!(!is_env_defined_falsy(Some(v)));
        }
        for v in ["0", " false ", "No", "OFF"] {
            assert!(is_env_defined_falsy(Some(v)));
            assert!(!is_env_truthy(Some(v)));
        }
        for v in ["", " ", "maybe", "trueish"] {
            assert!(!is_env_truthy(Some(v)));
            assert!(!is_env_defined_falsy(Some(v)));
        }
        assert!(!is_env_truthy(None));
        assert!(!is_env_defined_falsy(None));
    }
    #[test]
    fn parsing_preserves_embedded_equals_and_last_assignment() {
        let env = parse_env_vars(&["X=a=b=c".into(), "Y=".into(), "X=last".into()]).unwrap();
        assert_eq!(env["X"], "last");
        assert_eq!(env["Y"], "");
        assert!(parse_env_vars(&["A".into()]).is_err());
        assert!(parse_env_vars(&["=value".into()]).is_err());
        assert!(parse_env_vars(&[]).unwrap().is_empty());
    }
    #[test]
    fn argv_bare_mode_and_exact_options() {
        let args = vec!["app".into(), "--barely".into()];
        assert!(!bare_mode(Some("false"), &args));
        assert!(bare_mode(Some("true"), &args));
        assert!(bare_mode(None, &["--bare".into()]));
        assert!(has_process_option(Some("--loader x --flag=y"), "--loader"));
        assert!(!has_process_option(Some("--flag=yellow"), "--flag=y"));
    }
    #[test]
    fn config_directory_nfc_and_nested_teams() {
        let root = config_home_dir(Some("/tmp/cafe\u{301}")).unwrap();
        assert_eq!(root.to_string_lossy(), "/tmp/caf\u{e9}");
        assert_eq!(
            teams_dir(Some("/tmp/x")).unwrap(),
            PathBuf::from("/tmp/x/teams")
        );
    }
    #[test]
    fn region_fallback_order() {
        assert_eq!(aws_region(Some(""), Some("eu-central-1")), "eu-central-1");
        assert_eq!(aws_region(None, None), "us-east-1");
        assert_eq!(default_vertex_region(Some("")), "us-east5");
        assert_eq!(default_vertex_region(Some("europe-west4")), "europe-west4");
    }
}
