//! Rust exports derived from original: src/components/mcp/CapabilitiesSection.tsx
//!
//! Capability list construction: tools/resources/prompts appended in that
//! order when their counts are positive; the byline joins with " · " and
//! only renders when non-empty. Text layout rides the ink runtime.

/// `capabilities` array core.
pub fn capabilities_list(
    server_tools_count: u64,
    server_prompts_count: u64,
    server_resources_count: u64,
) -> Vec<&'static str> {
    let mut capabilities = Vec::new();
    if server_tools_count > 0 {
        capabilities.push("tools");
    }
    if server_resources_count > 0 {
        capabilities.push("resources");
    }
    if server_prompts_count > 0 {
        capabilities.push("prompts");
    }
    capabilities
}

/// Byline join (original composes `capabilities.join(" · ")`).
pub fn byline(counts: (u64, u64, u64)) -> Option<String> {
    let list = capabilities_list(counts.0, counts.2, counts.1);
    (!list.is_empty()).then(|| list.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_omission() {
        assert_eq!(
            capabilities_list(1, 2, 3),
            vec!["tools", "resources", "prompts"]
        );
        assert_eq!(capabilities_list(0, 0, 0), Vec::<&str>::new());
        assert_eq!(
            byline((1, 2, 3)).as_deref(),
            Some("tools · resources · prompts")
        );
        assert_eq!(byline((0, 0, 0)), None);
    }
}
