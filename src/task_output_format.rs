//! Task output display limits, independent of vendor-specific temp paths.
//! JavaScript strings count UTF-16 code units rather than UTF-8 bytes.
use crate::env_validation;

pub const TASK_MAX_OUTPUT_DEFAULT: usize = 32_000;
pub const TASK_MAX_OUTPUT_UPPER_LIMIT: usize = 160_000;

pub fn max_output_length() -> usize {
    let value = std::env::var("TASK_MAX_OUTPUT_LENGTH").ok();
    env_validation::validate("TASK_MAX_OUTPUT_LENGTH", value.as_deref(),
        TASK_MAX_OUTPUT_DEFAULT as u64, TASK_MAX_OUTPUT_UPPER_LIMIT as u64)
        .effective as usize
}

pub struct Formatted {
    pub content: String,
    pub was_truncated: bool,
}

fn unit_count(input: &str) -> usize {
    input.encode_utf16().count()
}

fn last_utf16_units(input: &str, count: usize) -> String {
    if count == 0 { return String::new(); }
    let units = input.encode_utf16().collect::<Vec<u16>>();
    if units.len() <= count { return input.to_string() }
    // Rust Unicode strings cannot represent a lone surrogate. That special
    // boundary case remains a documented compatibility gap.
    String::from_utf16_lossy(&units[units.len()-count..])
}

/// Format output using a workspace-local user-owned disk path.
pub fn format_task_output(output: &str, path: &str, max_len: usize) -> Formatted {
    if unit_count(output) <= max_len {
        return Formatted { content: output.into(), was_truncated: false };
    }
    let header = format!("[Truncated. Full output: {path}]\n\n");
    let available = max_len.saturating_sub(unit_count(&header));
    let mut content = header;
    content.push_str(&last_utf16_units(output, available));
    Formatted { content, was_truncated: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_boundary_and_unicode() {
        let old = format_task_output("abc", "/tmp/task.output", 5);
        assert_eq!(old.content, "abc");
        assert!(!old.was_truncated);
        let header = "[Truncated. Full output: a]\n\n";
        let result = format_task_output("abcdefghij", "a", header.len()+4);
        assert!(result.was_truncated);
        assert_eq!(result.content, format!("{header}ghij"));
        let emoji = format_task_output("A😀B", "a", 4);
        assert_eq!(emoji.content, "A😀B");
        assert!(!emoji.was_truncated);
        let short = format_task_output("abcdef", "a", header.len()+3);
        assert!(!short.was_truncated);
    }

    #[test]
    fn environment_limit_preserves_decimal_prefix() {
        for (input, expected) in [
            (None, 32_000), (Some("0"), 32_000), (Some(" 90tail"), 90),
            (Some("-1"), 32_000), (Some("160001"), 160_000),
        ] {
            let result = env_validation::validate("TASK_MAX_OUTPUT_LENGTH",
                input, TASK_MAX_OUTPUT_DEFAULT as u64, TASK_MAX_OUTPUT_UPPER_LIMIT as u64);
            assert_eq!(result.effective as usize, expected, "{input:?}");
        }
    }
}
