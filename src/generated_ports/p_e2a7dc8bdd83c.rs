//! Rust exports derived from original: src/hooks/useUpdateNotification.ts
//!
//! Full equivalence: semver-major.minor.patch extraction with loose parsing
//! (tolerates prerelease/build suffixes) and the notification trigger that
//! compares the extracted semver against the last notified one.

/// `getSemverPart` — "M.m.p" of the first three numeric components; unknown
/// parts read as 0, matching semver loose coercion on partial versions.
pub fn get_semver_part(version: &str) -> String {
    let nums: Vec<u64> = version
        .split(|c: char| c == '.' || c == '-' || c == '+')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect();
    let pick = |i: usize| nums.get(i).copied().unwrap_or(0);
    format!("{}.{}.{}", pick(0), pick(1), pick(2))
}

/// `shouldShowUpdateNotification` — notify when the update's semver part
/// differs from the last notified one.
pub fn should_show_update_notification(
    updated_version: &str,
    last_notified_semver: Option<&str>,
) -> bool {
    let updated = get_semver_part(updated_version);
    last_notified_semver != Some(updated.as_str())
}

/// `useUpdateNotification` state transition: seed the notified semver from
/// the initial version, then derive the notification decision for the new
/// version (None clears the notification without touching the seed).
pub fn update_notification_decision(
    updated_version: Option<&str>,
    last_notified_semver: Option<&str>,
) -> Option<String> {
    let updated = updated_version?;
    let semver = get_semver_part(updated);
    if Some(semver.as_str()) != last_notified_semver {
        Some(semver)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_semver_extraction() {
        assert_eq!(get_semver_part("2.1.88-beta.1"), "2.1.88");
        assert_eq!(get_semver_part("v3"), "3.0.0");
        assert_eq!(get_semver_part("1.2.3.4"), "1.2.3");
    }

    #[test]
    fn notification_triggers_on_semver_change() {
        assert!(should_show_update_notification("2.2.0", Some("2.1.88")));
        assert!(!should_show_update_notification(
            "2.1.88-rc.2",
            Some("2.1.88")
        ));
        assert!(should_show_update_notification("2.1.89", None));
    }
}
