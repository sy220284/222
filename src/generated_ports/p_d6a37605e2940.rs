//! Rust exports derived from original: src/costHook.ts
//!
//! Turn-end cost summary wiring: when the process owns console billing
//! access the formatted total is written to stdout, and the session costs
//! (with optional FPS metrics) are always saved. In this crate the cost
//! ledger lives in `cost` and the billing-access gate is a settings check;
//! the exit-hook registration is the CLI shutdown path's responsibility.

/// `useCostSummary` effect body — produces the summary block to append to
/// stdout when billing access exists, and returns the save action payload.
pub struct CostSummaryOutcome {
    /// Formatted total cost block (newline-wrapped) when billing access is
    /// available; None otherwise.
    pub stdout_block: Option<String>,
    /// Formatted total cost for the saved record (always computed).
    pub total_cost: String,
}

/// `formatTotalCost` + gate check collapsed into one decision point.
pub fn cost_summary(billing_access: bool, formatted_total: &str) -> CostSummaryOutcome {
    CostSummaryOutcome {
        stdout_block: if billing_access {
            Some(format!("\n{formatted_total}\n"))
        } else {
            None
        },
        total_cost: formatted_total.to_owned(),
    }
}
