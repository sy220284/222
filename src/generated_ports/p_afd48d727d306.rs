//! Rust exports derived from original: src/commands/cost/cost.ts
//!
//! The generic (non-subscriber) branch is fully equivalent: the command
//! returns the formatted total cost. The subscriber/overage branch belongs
//! to the vendor subscription surface, which is excluded from this port
//! (recorded difference, not behavior).

/// `call` generic-path text: the formatted total cost ledger summary.
pub fn generic_cost_text(formatted_total: &str) -> String {
    formatted_total.to_owned()
}

/// Command result shape used by local slash commands.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCommandResult {
    pub value: String,
}
