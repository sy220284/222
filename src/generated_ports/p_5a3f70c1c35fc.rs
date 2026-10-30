//! Rust exports derived from original: src/components/messages/UserPlanMessage.tsx
//!
//! "Plan to implement" header (bold, planMode color) with a conditional
//! bottom margin (addMargin ? 1 : 0); plan content renders as Markdown.

/// Header text.
pub const PLAN_HEADER: &str = "Plan to implement";

/// Bottom margin value for the header block.
pub fn header_margin(add_margin: bool) -> u16 {
    if add_margin {
        1
    } else {
        0
    }
}
