//! Rust exports derived from original: src/components/messages/UserBashOutputMessage.tsx
//!
//! Full equivalence at the data layer: stdout is the `bash-stdout` tag with
//! any `persisted-output` wrapper stripped; stderr is the `bash-stderr` tag;
//! missing tags read as empty. Tag extraction itself comes from the
//! messages port.

use crate::generated_ports::p_13ed07885469b::extract_tag;

pub struct UserBashOutput {
    pub stdout: String,
    pub stderr: String,
}

/// `UserBashOutputMessage` data prep before rendering.
pub fn user_bash_output(content: &str) -> UserBashOutput {
    let raw_stdout = extract_tag(content, "bash-stdout").unwrap_or_default();
    let stdout = extract_tag(&raw_stdout, "persisted-output").unwrap_or(raw_stdout);
    let stderr = extract_tag(content, "bash-stderr").unwrap_or_default();
    UserBashOutput { stdout, stderr }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_wrapper_and_reads_stderr() {
        let content = "<bash-stdout><persisted-output>saved</persisted-output></bash-stdout><bash-stderr>err</bash-stderr>";
        let out = user_bash_output(content);
        assert_eq!(out.stdout, "saved");
        assert_eq!(out.stderr, "err");
    }

    #[test]
    fn missing_tags_read_empty() {
        let out = user_bash_output("no tags");
        assert_eq!(out.stdout, "");
        assert_eq!(out.stderr, "");
    }
}
