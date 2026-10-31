//! Bounded integer environment validation with decimal-prefix parse semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationStatus {
    Valid,
    Capped,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedInt {
    pub effective: u64,
    pub status: ValidationStatus,
    pub message: Option<String>,
}

/// Accept a decimal prefix rather than Rust's strict integer parser: the
/// original contract relies on parseInt(value, 10).
pub(crate) fn decimal_prefix(value: &str) -> Option<u64> {
    let trimmed = value.trim_start_matches(char::is_whitespace);
    let (negative, digits) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let mut parsed = None;
    let mut overflow = false;
    let mut value = 0u64;
    for b in digits.bytes() {
        if !b.is_ascii_digit() {
            break;
        }
        parsed = Some(());
        let digit = u64::from(b - b'0');
        match value.checked_mul(10).and_then(|x| x.checked_add(digit)) {
            Some(x) => value = x,
            None => overflow = true,
        }
    }
    parsed?;
    if negative {
        return Some(0);
    }
    if overflow {
        Some(u64::MAX)
    } else {
        Some(value)
    }
}

pub fn validate(name: &str, value: Option<&str>, default: u64, upper_limit: u64) -> BoundedInt {
    let Some(raw) = value.filter(|v| !v.is_empty()) else {
        return BoundedInt {
            effective: default,
            status: ValidationStatus::Valid,
            message: None,
        };
    };
    let parsed = decimal_prefix(raw);
    if parsed.is_none() || parsed == Some(0) {
        return BoundedInt {
            effective: default,
            status: ValidationStatus::Invalid,
            message: Some(format!(
                "Invalid value \"{raw}\" (using default: {default})"
            )),
        };
    }
    let number = parsed.unwrap();
    if number > upper_limit {
        return BoundedInt {
            effective: upper_limit,
            status: ValidationStatus::Capped,
            message: Some(format!("Capped from {number} to {upper_limit}")),
        };
    }
    let _ = name; // logging is optional, never a hidden network event.
    BoundedInt {
        effective: number,
        status: ValidationStatus::Valid,
        message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_decimal_validation_matrix() {
        let cases = [
            (None, 32000, ValidationStatus::Valid),
            (Some(""), 32000, ValidationStatus::Valid),
            (Some("1"), 1, ValidationStatus::Valid),
            (Some("  +41 remainder"), 41, ValidationStatus::Valid),
            (Some("12.9"), 12, ValidationStatus::Valid),
            (Some("1e3"), 1, ValidationStatus::Valid),
            (Some("0xFF"), 32000, ValidationStatus::Invalid),
            (Some("0"), 32000, ValidationStatus::Invalid),
            (Some("-5"), 32000, ValidationStatus::Invalid),
            (Some(" "), 32000, ValidationStatus::Invalid),
            (Some("invalid"), 32000, ValidationStatus::Invalid),
            (Some("160001"), 160000, ValidationStatus::Capped),
        ];
        for (input, effective, status) in cases {
            let result = validate("TASK_MAX_OUTPUT_LENGTH", input, 32000, 160000);
            assert_eq!(result.effective, effective, "{input:?}");
            assert_eq!(result.status, status, "{input:?}");
        }
        let capped = validate("X", Some("200"), 5, 100);
        assert_eq!(capped.message.as_deref(), Some("Capped from 200 to 100"));
        let invalid = validate("X", Some("-1"), 5, 100);
        assert_eq!(
            invalid.message.as_deref(),
            Some("Invalid value \"-1\" (using default: 5)")
        );
    }
}
