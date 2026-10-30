//! Rust exports derived from original: src/components/tasks/renderToolActivity.tsx
//!
//! Rendering fallback chain: unknown tool -> raw name; unparseable input or
//! throwing renderer -> raw name; empty user-facing name -> raw name; args
//! present -> "Name(args)" else bare name. Text node composition rides the
//! ink runtime.

/// Text composition core (no React): returns what the view would print.
pub fn render_tool_activity_text(
    tool_name: &str,
    user_facing_name: Option<&str>,
    tool_args: Option<&str>,
) -> String {
    match (user_facing_name, tool_args) {
        (None, _) => tool_name.to_owned(),
        (Some(name), Some(args)) if !args.is_empty() => format!("{name}({args})"),
        (Some(name), _) => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_chain() {
        assert_eq!(render_tool_activity_text("Bash", None, None), "Bash");
        assert_eq!(
            render_tool_activity_text("Bash", Some(""), Some("x")),
            "Bash"
        );
        assert_eq!(
            render_tool_activity_text("Bash", Some("Bash"), Some("ls")),
            "Bash(ls)"
        );
        assert_eq!(
            render_tool_activity_text("Bash", Some("Bash"), Some("")),
            "Bash"
        );
    }
}
