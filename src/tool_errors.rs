//! Source-aligned tool error formatting: src/utils/toolErrors.ts.
use serde_json::Value;

pub fn format_error_text(message: &str) -> String {
    let message = message.trim();
    let message = if message.is_empty() {
        "Command failed with no output"
    } else {
        message
    };
    let len = message.chars().count();
    if len <= 10_000 {
        return message.to_owned();
    }
    let head: String = message.chars().take(5_000).collect();
    let tail: String = message.chars().skip(len - 5_000).collect();
    format!("{head}\n\n... [{} characters truncated] ...\n\n{tail}", len - 10_000)
}

pub fn shell_error_parts(
    exit_code: i32,
    interrupted: bool,
    stderr: &str,
    stdout: &str,
) -> Vec<String> {
    vec![
        format!("Exit code {exit_code}"),
        if interrupted {
            "[Request interrupted by user for tool use]".to_owned()
        } else {
            String::new()
        },
        stderr.to_owned(),
        stdout.to_owned(),
    ]
}

pub fn format_error_parts(parts: &[String]) -> String {
    format_error_text(
        &parts
            .iter()
            .filter(|p| !p.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn validation_path(path: &Value) -> String {
    let mut result = String::new();
    if let Some(path) = path.as_array() {
        for (index, segment) in path.iter().enumerate() {
            if let Some(n) = segment.as_i64() {
                result.push_str(&format!("[{n}]"));
            } else {
                let key = segment
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| segment.to_string());
                if index > 0 {
                    result.push('.');
                }
                result.push_str(&key);
            }
        }
    }
    result
}

/// Recreates source formatZodValidationError using provider-neutral JSON
/// issue objects. The 2.1.88 Zod issue schema is accepted unchanged.
pub fn format_validation_error(tool_name: &str, issues: &[Value], original: &str) -> String {
    let mut parts = Vec::new();
    for issue in issues {
        let code = issue.get("code").and_then(Value::as_str).unwrap_or("");
        let message = issue.get("message").and_then(Value::as_str).unwrap_or("");
        let path = validation_path(issue.get("path").unwrap_or(&Value::Null));
        match code {
            "invalid_type" if message.contains("received undefined") => {
                parts.push(format!("The required parameter '{path}' is missing"));
            }
            "unrecognized_keys" => {
                if let Some(keys) = issue.get("keys").and_then(Value::as_array) {
                    for key in keys.iter().filter_map(Value::as_str) {
                        parts.push(format!("An unexpected parameter '{key}' was provided"));
                    }
                }
            }
            "invalid_type" => {
                let expected = issue
                    .get("expected")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let received = message
                    .split_once("received ")
                    .and_then(|(_, text)| {
                        text.split(|c: char| !c.is_ascii_alphanumeric()).next()
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or("unknown");
                parts.push(format!(
                    "The parameter '{path}' type is expected as '{expected}' but provided as '{received}'"
                ));
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        return original.to_owned();
    }
    let quantity = if parts.len() == 1 { "issue" } else { "issues" };
    format!("{tool_name} failed due to the following {quantity}:\n{}", parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn large_errors_preserve_head_tail_and_empty_errors_have_fallback() {
        assert_eq!(format_error_text("  "), "Command failed with no output");
        let original = "A".repeat(6_000) + &"B".repeat(6_000);
        let result = format_error_text(&original);
        assert!(result.starts_with(&"A".repeat(5_000)));
        assert!(result.contains("[2000 characters truncated]"));
        assert!(result.ends_with(&"B".repeat(5_000)));
        assert_eq!(format_error_text("中文🌟"), "中文🌟");
    }

    #[test]
    fn source_shell_error_fields_are_ordered_and_visible() {
        let data = shell_error_parts(2, true, "stderr", "stdout");
        assert_eq!(
            format_error_parts(&data),
            "Exit code 2\n[Request interrupted by user for tool use]\nstderr\nstdout"
        );
    }

    #[test]
    fn validation_error_uses_nested_paths_and_original_fallback() {
        let issues = vec![
            json!({"code":"invalid_type","path":["todos",0,"activeForm"],"message":"Invalid input: received undefined"}),
            json!({"code":"unrecognized_keys","keys":["bad"]}),
            json!({"code":"invalid_type","path":["count"],"expected":"number","message":"expected number, received string"}),
        ];
        let output = format_validation_error("TaskUpdate", &issues, "original error");
        assert!(output.contains("todos[0].activeForm"));
        assert!(output.contains("bad"));
        assert!(output.contains("count"));
        assert!(output.contains("3 issues"));
        assert_eq!(format_validation_error("X", &[], "original"), "original");
    }
}
