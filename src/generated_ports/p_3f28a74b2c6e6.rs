//! Native Rust port derived from original: src/services/autoDream/config.ts
//!
//! Whether background memory consolidation runs. User setting
//! (`autoDreamEnabled` in settings.json) overrides the experiment
//! default only when explicitly set; otherwise the experiment value
//! decides. The settings loader and experiment source live behind
//! their own ports, so the decision logic is a pure function and the
//! caller supplies the two inputs.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

/// Decision core: `setting` is the parsed `autoDreamEnabled` from
/// settings.json (None = unset), `experiment_enabled` is the resolved
/// experiment value (false when the experiment is off/absent).
pub fn is_auto_dream_enabled(setting: Option<bool>, experiment_enabled: bool) -> bool {
    match setting {
        Some(v) => v,
        None => experiment_enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_setting_overrides_experiment() {
        assert!(is_auto_dream_enabled(Some(true), false));
        assert!(!is_auto_dream_enabled(Some(false), true));
    }

    #[test]
    fn unset_setting_falls_through_to_experiment() {
        assert!(is_auto_dream_enabled(None, true));
        assert!(!is_auto_dream_enabled(None, false));
    }
}
