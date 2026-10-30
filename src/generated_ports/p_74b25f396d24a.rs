//! Rust exports derived from original: src/cli/handlers/agents.ts
//!
//! Full equivalence: the agents list line format — `agentType`, optional
//! resolved model display, optional memory suffix, joined with " · ".

/// `formatAgent`.
pub fn format_agent(agent_type: &str, model_display: Option<&str>, memory: Option<&str>) -> String {
    let mut parts = vec![agent_type.to_owned()];
    if let Some(m) = model_display {
        parts.push(m.to_owned());
    }
    if let Some(m) = memory {
        parts.push(format!("{m} memory"));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_sections() {
        assert_eq!(format_agent("arch", None, None), "arch");
        assert_eq!(format_agent("arch", Some("opus"), None), "arch · opus");
        assert_eq!(format_agent("arch", None, Some("8k")), "arch · 8k memory");
        assert_eq!(
            format_agent("arch", Some("opus"), Some("8k")),
            "arch · opus · 8k memory"
        );
    }
}
