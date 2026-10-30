//! Rust exports derived from original: src/components/sandbox/SandboxDoctorSection.tsx
//!
//! Doctor section visibility decision: hidden on unsupported platforms,
//! when sandbox is off in settings, and when dependency checks are fully
//! clean; errors and warnings render as separate blocks.

pub struct DepCheck {
    pub errors: usize,
    pub warnings: usize,
}

/// Which blocks the doctor section renders, or None when the section is
/// hidden entirely.
pub fn doctor_blocks(
    platform_supported: bool,
    enabled_in_settings: bool,
    check: &DepCheck,
) -> Option<(&'static str, &'static str)> {
    if !platform_supported || !enabled_in_settings {
        return None;
    }
    if check.errors == 0 && check.warnings == 0 {
        return None;
    }
    let errors = if check.errors > 0 { "errors" } else { "" };
    let warnings = if check.warnings > 0 { "warnings" } else { "" };
    Some((errors, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_gates() {
        let clean = DepCheck {
            errors: 0,
            warnings: 0,
        };
        let mixed = DepCheck {
            errors: 1,
            warnings: 2,
        };
        assert!(doctor_blocks(false, true, &mixed).is_none());
        assert!(doctor_blocks(true, false, &mixed).is_none());
        assert!(doctor_blocks(true, true, &clean).is_none());
        let (e, w) = doctor_blocks(true, true, &mixed).unwrap();
        assert_eq!((e, w), ("errors", "warnings"));
    }
}
