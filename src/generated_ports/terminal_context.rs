//! Native input for `src/hooks/useTerminalSize.ts` and the terminal-size
//! context. Rust callers explicitly provide a context value and receive the
//! same missing-provider error instead of a fabricated 80x24 terminal.
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingTerminalSizeContext;
impl fmt::Display for MissingTerminalSizeContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "useTerminalSize must be used within an Ink App component"
        )
    }
}
impl std::error::Error for MissingTerminalSizeContext {}

/// An intentionally direct translation of the hook's observable behavior:
/// return the existing context object unchanged or report its precise error.
pub fn use_terminal_size(context: Option<&Value>) -> Result<Value, MissingTerminalSizeContext> {
    match context {
        Some(value) if crate::generated_ports::runtime::truthy(value) => Ok(value.clone()),
        _ => Err(MissingTerminalSizeContext),
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn provider_is_required_and_existing_dimensions_are_unchanged() {
        assert_eq!(use_terminal_size(None), Err(MissingTerminalSizeContext));
        assert_eq!(
            use_terminal_size(Some(&Value::Null))
                .unwrap_err()
                .to_string(),
            "useTerminalSize must be used within an Ink App component"
        );
        let size = json!({"columns":113,"rows":39});
        assert_eq!(use_terminal_size(Some(&size)), Ok(size));
    }
}
