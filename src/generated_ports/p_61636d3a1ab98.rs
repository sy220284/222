//! Rust exports derived from original: src/utils/awsAuthStatusManager.ts
//!
//! Cloud-provider auth status singleton (AWS Bedrock / GCP Vertex — provider
//! agnostic 'auth_status' shape). Lifecycle: begin → output lines → finish
//! (error optional) → reset. Change signal rides the reactive host.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AwsAuthStatus {
    pub is_authenticating: bool,
    pub output: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct AwsAuthStatusManager {
    status: AwsAuthStatus,
}

impl AwsAuthStatusManager {
    pub fn snapshot(&self) -> &AwsAuthStatus {
        &self.status
    }

    /// `begin` — enter authenticating state with a clean output buffer.
    pub fn begin(&mut self) -> &AwsAuthStatus {
        self.status = AwsAuthStatus {
            is_authenticating: true,
            output: vec![],
            error: None,
        };
        &self.status
    }

    /// `addOutput` — append a refresh output line.
    pub fn push_line(&mut self, line: String) {
        self.status.output.push(line);
    }

    /// `finish` — leave authenticating state; error stays visible.
    /// Returns (was_authenticating, has_error).
    pub fn finish(&mut self, error: Option<String>) -> (bool, bool) {
        let was = self.status.is_authenticating;
        self.status.is_authenticating = false;
        self.status.error = error;
        (was, self.status.error.is_some())
    }

    /// `reset` — full state reset.
    pub fn reset(&mut self) {
        self.status = AwsAuthStatus::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_semantics() {
        let mut m = AwsAuthStatusManager::default();
        m.begin();
        assert!(m.snapshot().is_authenticating);
        m.push_line("assuming role...".into());
        let (was, has_err) = m.finish(Some("expired".into()));
        assert!(was);
        assert!(has_err);
        assert_eq!(m.snapshot().error.as_deref(), Some("expired"));
        assert!(!m.snapshot().is_authenticating);
        m.reset();
        assert_eq!(m.snapshot(), &AwsAuthStatus::default());
    }
}
