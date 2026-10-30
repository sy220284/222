//! Rust exports derived from original: src/commands/rewind/rewind.ts
//!
//! Full equivalence: the command opens the message selector through the
//! tool-use context and returns a skip result so no messages are appended.

/// `LocalCommandResult` skip variant used by the command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LocalCommandResult {
    Skip,
}

/// `call` — the whole command body.
pub fn call(open_message_selector: Option<&mut dyn FnMut()>) -> LocalCommandResult {
    if let Some(open) = open_message_selector {
        open();
    }
    LocalCommandResult::Skip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_skip_and_invokes_selector() {
        let mut called = 0;
        let result = call(Some(&mut || called += 1));
        assert_eq!(result, LocalCommandResult::Skip);
        assert_eq!(called, 1);
        let result = call(None);
        assert_eq!(result, LocalCommandResult::Skip);
    }
}
