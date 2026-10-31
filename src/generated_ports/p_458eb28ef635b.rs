//! Native Rust port derived from original: src/commands/plugin/ValidatePlugin.tsx
//!
//! /plugin validate output assembly. The validation runner (validateManifest)
//! is injected as a result value; this port owns the complete report
//! formatting verbatim — usage text when no path, file-type header,
//! cross/warning/tick figures with plural handling, exit codes
//! (0 pass / 1 fail / 2 unexpected error).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

pub const USAGE_TEXT: &str = "Usage: /plugin validate <path>\n\nValidate a plugin or marketplace manifest file or directory.\n\nExamples:\n  /plugin validate .vendor-plugin/plugin.json\n  /plugin validate /path/to/plugin-directory\n  /plugin validate .\n\nWhen given a directory, automatically validates .vendor-plugin/marketplace.json\nor .vendor-plugin/plugin.json (prefers marketplace if both exist).\n\nOr from the command line:\n  upstream plugin validate <path>";

#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub file_type: String,
    pub file_path: String,
    pub errors: Vec<(String, String)>,
    pub warnings: Vec<(String, String)>,
    pub success: bool,
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// Report formatter (original runValidation body, verbatim strings).
pub fn format_validation_report(result: &ValidationResult) -> String {
    let mut out = format!(
        "Validating {} manifest: {}\n\n",
        result.file_type, result.file_path
    );
    if !result.errors.is_empty() {
        out += &format!(
            "\u{2717} Found {} {}:\n\n",
            result.errors.len(),
            plural(result.errors.len(), "error")
        );
        for (path, message) in &result.errors {
            out += &format!("  \u{2192} {path}: {message}\n");
        }
        out += "\n";
    }
    if !result.warnings.is_empty() {
        out += &format!(
            "\u{26a0} Found {} {}:\n\n",
            result.warnings.len(),
            plural(result.warnings.len(), "warning")
        );
        for (path, message) in &result.warnings {
            out += &format!("  \u{2192} {path}: {message}\n");
        }
        out += "\n";
    }
    if result.success {
        if !result.warnings.is_empty() {
            out += "\u{2713} Validation passed with warnings\n";
        } else {
            out += "\u{2713} Validation passed\n";
        }
    } else {
        out += "\u{2717} Validation failed\n";
    }
    out
}

/// Exit code contract: 0 pass / 1 fail / 2 unexpected error.
pub fn exit_code_for(result: Option<&ValidationResult>) -> i32 {
    match result {
        Some(r) if r.success => 0,
        Some(_) => 1,
        None => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_path_shows_usage() {
        // The original completes with USAGE_TEXT when path is absent.
        assert!(USAGE_TEXT.starts_with("Usage: /plugin validate <path>"));
        assert!(USAGE_TEXT.contains("prefers marketplace if both exist"));
    }

    #[test]
    fn report_formats_errors_warnings_and_verdict() {
        let result = ValidationResult {
            file_type: "plugin".into(),
            file_path: "/p/plugin.json".into(),
            errors: vec![("commands[0].name".into(), "too long".into())],
            warnings: vec![("author".into(), "missing email".into())],
            success: false,
        };
        let report = format_validation_report(&result);
        assert!(report.contains("Validating plugin manifest: /p/plugin.json"));
        assert!(report.contains("\u{2717} Found 1 error:"));
        assert!(report.contains("\u{2192} commands[0].name: too long"));
        assert!(report.contains("\u{26a0} Found 1 warning:"));
        assert!(report.contains("\u{2717} Validation failed"));
        assert_eq!(exit_code_for(Some(&result)), 1);
    }

    #[test]
    fn pass_with_warnings_message_differs() {
        let result = ValidationResult {
            file_type: "marketplace".into(),
            file_path: "/m.json".into(),
            errors: vec![],
            warnings: vec![("a".into(), "b".into())],
            success: true,
        };
        assert!(
            format_validation_report(&result).contains("\u{2713} Validation passed with warnings")
        );
        assert_eq!(exit_code_for(Some(&result)), 0);
    }

    #[test]
    fn unexpected_error_exit_code() {
        assert_eq!(exit_code_for(None), 2);
    }
}
