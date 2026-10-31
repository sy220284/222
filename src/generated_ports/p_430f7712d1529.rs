//! Rust exports derived from original: src/constants/system.ts
//!
//! The three CLI system-prompt prefixes are brand-bearing strings excluded
//! by the privacy gate; their runtime equivalents are injected by
//! prompt_runtime (see src/prompt_runtime.rs). What ports losslessly is the
//! prefix-identification decision: splitSysPromptPrefix matches a leading
//! block by content, not position, and the SDK preset variant selects the
//! middle prefix.

/// Which prefix a leading assistant block should be recognized as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliSyspromptPrefix {
    Default,
    AgentSdkPreset,
    AgentSdk,
}

/// `getCLISyspromptPrefix` — SDK preset switches between the first two
/// variants; everything else falls back to the default.
pub fn get_cli_sysprompt_prefix(sdk_preset: bool) -> CliSyspromptPrefix {
    if sdk_preset {
        CliSyspromptPrefix::AgentSdkPreset
    } else {
        CliSyspromptPrefix::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_selection() {
        assert_eq!(get_cli_sysprompt_prefix(false), CliSyspromptPrefix::Default);
        assert_eq!(
            get_cli_sysprompt_prefix(true),
            CliSyspromptPrefix::AgentSdkPreset
        );
    }
}
