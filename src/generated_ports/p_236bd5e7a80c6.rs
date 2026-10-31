//! Rust exports derived from original: src/services/awaySummary.ts
//!
//! Full equivalence at the prompt layer: the away-summary instruction is
//! verbatim, with the optional session-memory block and the 30-message
//! recent-context window constant. Model invocation and abort handling ride
//! the model chain.

/// `RECENT_MESSAGE_WINDOW` — ~15 exchanges is plenty for a recap.
pub const RECENT_MESSAGE_WINDOW: usize = 30;

/// `buildAwaySummaryPrompt` — memory block prepended when present.
pub fn build_away_summary_prompt(memory: Option<&str>) -> String {
    let memory_block = match memory {
        Some(m) => format!("Session memory (broader context):\n{m}\n\n"),
        None => String::new(),
    };
    format!(
        "{memory_block}The user stepped away and is coming back. Write exactly 1-3 short sentences. Start by stating the high-level task — what they are building or debugging, not implementation details. Next: the concrete next step. Skip status reports and commit recaps."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_with_and_without_memory() {
        let a = build_away_summary_prompt(None);
        assert!(!a.contains("Session memory"));
        let b = build_away_summary_prompt(Some("ctx"));
        assert!(b.starts_with("Session memory (broader context):\nctx\n\n"));
        assert!(a.ends_with("Skip status reports and commit recaps."));
    }
}
