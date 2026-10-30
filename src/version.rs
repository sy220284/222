//! Original src/commands/version.ts, adapted to a provider-neutral binary.
//! The reported version belongs to this running executable, not to a
//! separately downloaded updater artifact.

pub fn running_version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    match option_env!("ATLAS_BUILD_TIME") {
        Some(timestamp) if !timestamp.is_empty() => {
            format!("{version} (built {timestamp})")
        }
        _ => version.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_from_running_binary_and_build_time_is_optional() {
        let version = running_version();
        assert!(version.starts_with(env!("CARGO_PKG_VERSION")));
        assert!(!version.contains("unknown"));
        if let Some(timestamp) = option_env!("ATLAS_BUILD_TIME") {
            if !timestamp.is_empty() {
                assert_eq!(
                    version,
                    format!("{} (built {timestamp})", env!("CARGO_PKG_VERSION"))
                );
            }
        }
    }
}
