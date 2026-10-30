//! Rust exports derived from original: src/constants/keys.ts
//!
//! Full equivalence: the client key selection is a pure three-way branch on
//! the same environment inputs (USER_TYPE build define + lazy truthy read).

/// `getGrowthBookClientKey` — lazy read so ENABLE_GROWTHBOOK_DEV from
/// globalSettings.env (applied after module load) is picked up. USER_TYPE is
/// a build-time define so it is read as-is.
pub fn get_growthbook_client_key() -> &'static str {
    let user_type_is_ant = std::env::var("USER_TYPE").ok().as_deref() == Some("ant");
    let dev =
        crate::env_runtime::is_env_truthy(std::env::var("ENABLE_GROWTHBOOK_DEV").ok().as_deref());
    if user_type_is_ant {
        if dev {
            "sdk-yZQvlplybuXjYh6L"
        } else {
            "sdk-xRVcrliHIlrg4og4"
        }
    } else {
        "sdk-zAZezfDKGoZuXXKe"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_ant_cohort_uses_shared_key() {
        std::env::remove_var("USER_TYPE");
        std::env::remove_var("ENABLE_GROWTHBOOK_DEV");
        assert_eq!(super::get_growthbook_client_key(), "sdk-zAZezfDKGoZuXXKe");
    }

    #[test]
    fn ant_cohort_switches_on_dev_flag() {
        std::env::set_var("USER_TYPE", "ant");
        std::env::remove_var("ENABLE_GROWTHBOOK_DEV");
        assert_eq!(super::get_growthbook_client_key(), "sdk-xRVcrliHIlrg4og4");
        std::env::set_var("ENABLE_GROWTHBOOK_DEV", "1");
        assert_eq!(super::get_growthbook_client_key(), "sdk-yZQvlplybuXjYh6L");
        std::env::remove_var("USER_TYPE");
        std::env::remove_var("ENABLE_GROWTHBOOK_DEV");
    }
}
