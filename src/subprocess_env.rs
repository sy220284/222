//! Central subprocess environment scrub, applied after subprocess-specific
//! environment overrides so user-provided values cannot bypass isolation.
//! This is opt-in, mirroring the original guarded GitHub Actions behavior,
//! but provider-neutral rather than relying on vendor account tokens.
use tokio::process::Command;

const SENSITIVE: &[&str] = &[
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_CLIENT_SECRET",
    "AZURE_CLIENT_CERTIFICATE_PATH",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_BEARER_TOKEN_BEDROCK",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "OTEL_EXPORTER_OTLP_HEADERS",
    "OTEL_EXPORTER_OTLP_LOGS_HEADERS",
    "OTEL_EXPORTER_OTLP_METRICS_HEADERS",
    "OTEL_EXPORTER_OTLP_TRACES_HEADERS",
    "ACTIONS_ID_TOKEN_REQUEST_TOKEN",
    "ACTIONS_ID_TOKEN_REQUEST_URL",
    "ACTIONS_RUNTIME_TOKEN",
    "ACTIONS_RUNTIME_URL",
    "ALL_INPUTS",
    "OVERRIDE_GITHUB_TOKEN",
    "DEFAULT_WORKFLOW_TOKEN",
    "SSH_SIGNING_KEY",
];

fn scrub_enabled() -> bool {
    crate::env_runtime::is_env_truthy(std::env::var("ATLAS_SUBPROCESS_ENV_SCRUB").ok().as_deref())
}

/// Applies to a prepared process, after all custom environment injection.
pub fn apply(cmd: &mut Command) {
    apply_with_flag(cmd, scrub_enabled());
}
fn apply_with_flag(cmd: &mut Command, enabled: bool) {
    if !enabled {
        return;
    }
    for &key in SENSITIVE {
        cmd.env_remove(key);
        cmd.env_remove(format!("INPUT_{key}"));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scrub_flag_parses_only_explicit_truth() {
        assert!(!matches!("0", "1" | "true" | "yes" | "on"));
        assert!(matches!("true", "1" | "true" | "yes" | "on"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn secrets_are_suppressed_only_for_selected_children() {
        let mut process = Command::new("sh");
        process
            .args([
                "-c",
                "printf '%s' \"$OPENAI_API_KEY|$INPUT_OPENAI_API_KEY|$GH_TOKEN\"",
            ])
            .env("OPENAI_API_KEY", "hidden")
            .env("INPUT_OPENAI_API_KEY", "duplicate")
            .env("GH_TOKEN", "job-scoped");
        apply_with_flag(&mut process, true);
        let output = process.output().await.unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "||job-scoped");
        let mut normal = Command::new("sh");
        normal
            .args(["-c", "printf '%s' \"$OPENAI_API_KEY\""])
            .env("OPENAI_API_KEY", "preserved");
        apply_with_flag(&mut normal, false);
        let output = normal.output().await.unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "preserved");
    }
}
