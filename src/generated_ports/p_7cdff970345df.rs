//! Rust exports derived from original: src/services/tips/tipScheduler.ts
//!
//! The selection core is fully equivalent and tested: pick the tip with the
//! longest time since last shown (stable order for ties via the original's
//! sort). History accessors and vendor analytics live in their own ports.

use serde::{Deserialize, Serialize};

/// `Tip` minimal shape used by the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tip {
    pub id: String,
}

/// `selectTipWithLongestTimeSinceShown` — descending by sessions since shown,
/// first match wins; empty input yields None.
pub fn select_tip_longest_unshown(
    available: &[Tip],
    sessions_since_last_shown: impl Fn(&str) -> u64,
) -> Option<Tip> {
    let mut best: Option<(u64, &Tip)> = None;
    for tip in available {
        let n = sessions_since_last_shown(&tip.id);
        let take = match best {
            None => true,
            Some((top, _)) => n > top,
        };
        if take {
            best = Some((n, tip));
        }
    }
    best.map(|(_, tip)| tip.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip(id: &str) -> Tip {
        Tip { id: id.to_owned() }
    }

    #[test]
    fn empty_yields_none_single_short_circuits() {
        assert!(select_tip_longest_unshown(&[], |_| 0).is_none());
        let one = [tip("a")];
        assert_eq!(select_tip_longest_unshown(&one, |_| 0).unwrap().id, "a");
    }

    #[test]
    fn picks_longest_gap_first_match_on_ties() {
        let tips = [tip("a"), tip("b"), tip("c")];
        let counts = |id: &str| match id {
            "a" => 3,
            "b" => 7,
            _ => 7,
        };
        assert_eq!(select_tip_longest_unshown(&tips, counts).unwrap().id, "b");
    }
}
