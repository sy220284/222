//! Rust exports derived from original: src/utils/iTermBackup.ts
//!
//! Full equivalence at the state layer: the setup-in-progress flag clears on
//! completion, and recovery reads the flag plus the last backup path from
//! config. File copy and the plist path (~/Library/Preferences/
//! com.googlecode.iterm2.plist) ride the config/fs chain.

/// `markITerm2SetupComplete` state effect.
pub fn mark_setup_complete(in_progress: bool) -> bool {
    let _ = in_progress;
    false
}

/// `getIterm2RecoveryInfo` projection.
pub fn recovery_info(in_progress: bool, backup_path: Option<&str>) -> (bool, Option<String>) {
    (in_progress, backup_path.map(str::to_owned))
}

/// `getITerm2PlistPath` — macOS preferences location.
pub fn iterm2_plist_path(home: &str) -> String {
    format!("{home}/Library/Preferences/com.googlecode.iterm2.plist")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_and_path() {
        assert_eq!(mark_setup_complete(true), false);
        assert_eq!(recovery_info(true, Some("/b")), (true, Some("/b".into())));
        assert_eq!(
            iterm2_plist_path("/Users/x"),
            "/Users/x/Library/Preferences/com.googlecode.iterm2.plist"
        );
    }
}
