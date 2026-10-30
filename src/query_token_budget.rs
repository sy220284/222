//! Rust port of `src/query/tokenBudget.ts` (2.1.88 original implementation).
//!
//! A user can attach an output-token target to a prompt (e.g. `+500k`).
//! While the budget is not yet 90% consumed, the query loop keeps the model
//! working by feeding a meta continuation nudge; once the budget is reached
//! or progress becomes diminishing, the turn stops and reports a completion
//! event. Branch layout, thresholds and the exact nudge wording follow the
//! original implementation line-for-line.

use crate::portable_source_utils::budget_continuation_message;

/// `COMPLETION_THRESHOLD` — fraction of the budget at which the turn counts
/// as complete.
const COMPLETION_THRESHOLD: f64 = 0.9;
/// `DIMINISHING_THRESHOLD` — output-token delta below which two consecutive
/// checks count as diminishing returns.
const DIMINISHING_THRESHOLD: f64 = 500.0;

/// `BudgetTracker` — loop-local bookkeeping created once per `query()` call.
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    pub continuation_count: u32,
    pub last_delta_tokens: f64,
    pub last_global_turn_tokens: f64,
    started_at: std::time::Instant,
}

/// `createBudgetTracker()`.
pub fn create_budget_tracker() -> BudgetTracker {
    BudgetTracker {
        continuation_count: 0,
        last_delta_tokens: 0.0,
        last_global_turn_tokens: 0.0,
        started_at: std::time::Instant::now(),
    }
}

/// `ContinueDecision`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinueDecision {
    pub nudge_message: String,
    pub continuation_count: u32,
    pub pct: i64,
    pub turn_tokens: f64,
    pub budget: f64,
}

/// `completionEvent` carried by a stopping decision.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetCompletionEvent {
    pub continuation_count: u32,
    pub pct: i64,
    pub turn_tokens: f64,
    pub budget: f64,
    pub diminishing_returns: bool,
    pub duration_ms: u64,
}

/// `TokenBudgetDecision` — `ContinueDecision | StopDecision` in the original.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenBudgetDecision {
    Continue(ContinueDecision),
    Stop {
        completion_event: Option<BudgetCompletionEvent>,
    },
}

/// `Intl.NumberFormat('en-US')` grouping for token counts, used by the
/// continuation debug line (`toLocaleString()` in the original).
pub(crate) fn format_token_count(n: f64) -> String {
    let digits = if n.fract() == 0.0 {
        (n as u64).to_string()
    } else {
        return n.to_string();
    };
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `Math.round` — ties round towards positive infinity (`floor(x + 0.5)`),
/// which differs from Rust's `f64::round` (ties away from zero) for the
/// values in play here; matching JavaScript keeps `pct` byte-identical.
fn js_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// `checkTokenBudget` — decide whether the current turn keeps working under
/// its output-token budget or stops.
///
/// * `agent_id` — present on subagent turns; the original skips budget
///   enforcement entirely for them.
/// * `budget` — the turn's output-token target; `None`/non-positive disables
///   the mechanism.
/// * `global_turn_tokens` — output tokens produced so far this turn
///   (`getTurnOutputTokens()`).
pub fn check_token_budget(
    tracker: &mut BudgetTracker,
    agent_id: Option<&str>,
    budget: Option<f64>,
    global_turn_tokens: f64,
) -> TokenBudgetDecision {
    if agent_id.is_some() || budget.is_none_or(|b| b <= 0.0) {
        return TokenBudgetDecision::Stop {
            completion_event: None,
        };
    }
    let budget = budget.unwrap_or(0.0);

    let turn_tokens = global_turn_tokens;
    let pct = js_round((turn_tokens / budget) * 100.0);
    let delta_since_last_check = global_turn_tokens - tracker.last_global_turn_tokens;

    let is_diminishing = tracker.continuation_count >= 3
        && delta_since_last_check < DIMINISHING_THRESHOLD
        && tracker.last_delta_tokens < DIMINISHING_THRESHOLD;

    if !is_diminishing && turn_tokens < budget * COMPLETION_THRESHOLD {
        tracker.continuation_count += 1;
        tracker.last_delta_tokens = delta_since_last_check;
        tracker.last_global_turn_tokens = global_turn_tokens;
        return TokenBudgetDecision::Continue(ContinueDecision {
            nudge_message: budget_continuation_message(
                pct.max(0) as u32,
                turn_tokens as u64,
                budget as u64,
            ),
            continuation_count: tracker.continuation_count,
            pct,
            turn_tokens,
            budget,
        });
    }

    if is_diminishing || tracker.continuation_count > 0 {
        return TokenBudgetDecision::Stop {
            completion_event: Some(BudgetCompletionEvent {
                continuation_count: tracker.continuation_count,
                pct,
                turn_tokens,
                budget,
                diminishing_returns: is_diminishing,
                duration_ms: tracker.started_at.elapsed().as_millis() as u64,
            }),
        };
    }

    TokenBudgetDecision::Stop {
        completion_event: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> BudgetTracker {
        create_budget_tracker()
    }

    #[test]
    fn subagent_turns_never_continue() {
        let mut t = tracker();
        assert_eq!(
            check_token_budget(&mut t, Some("agent-1"), Some(1000.0), 10.0),
            TokenBudgetDecision::Stop {
                completion_event: None
            }
        );
    }

    #[test]
    fn missing_or_nonpositive_budget_stops_silently() {
        let mut t = tracker();
        assert_eq!(
            check_token_budget(&mut t, None, None, 10.0),
            TokenBudgetDecision::Stop {
                completion_event: None
            }
        );
        assert_eq!(
            check_token_budget(&mut t, None, Some(0.0), 10.0),
            TokenBudgetDecision::Stop {
                completion_event: None
            }
        );
        assert_eq!(
            check_token_budget(&mut t, None, Some(-5.0), 10.0),
            TokenBudgetDecision::Stop {
                completion_event: None
            }
        );
    }

    #[test]
    fn below_threshold_continues_with_nudge() {
        let mut t = tracker();
        match check_token_budget(&mut t, None, Some(1000.0), 100.0) {
            TokenBudgetDecision::Continue(c) => {
                assert_eq!(c.continuation_count, 1);
                assert_eq!(c.pct, 10);
                assert_eq!(c.turn_tokens, 100.0);
                assert_eq!(c.budget, 1000.0);
                assert_eq!(
                    c.nudge_message,
                    "Stopped at 10% of token target (100 / 1,000). Keep working — do not summarize."
                );
            }
            other => panic!("expected continue, got {other:?}"),
        }
    }

    #[test]
    fn reaching_ninety_percent_stops_with_event() {
        let mut t = tracker();
        // 90% is not < 90% → falls through; continuation_count is 0 → no event
        assert_eq!(
            check_token_budget(&mut t, None, Some(1000.0), 900.0),
            TokenBudgetDecision::Stop {
                completion_event: None
            }
        );
        // one continuation first, then 90% → stop with completion event
        assert!(matches!(
            check_token_budget(&mut t, None, Some(1000.0), 100.0),
            TokenBudgetDecision::Continue(_)
        ));
        match check_token_budget(&mut t, None, Some(1000.0), 900.0) {
            TokenBudgetDecision::Stop {
                completion_event: Some(e),
            } => {
                assert_eq!(e.continuation_count, 1);
                assert_eq!(e.pct, 90);
                assert_eq!(e.turn_tokens, 900.0);
                assert_eq!(e.budget, 1000.0);
                assert!(!e.diminishing_returns);
            }
            other => panic!("expected stop with event, got {other:?}"),
        }
    }

    #[test]
    fn three_stalled_continuations_trigger_diminishing_returns() {
        let mut t = tracker();
        for tokens in [100.0, 500.0, 900.0] {
            assert!(matches!(
                check_token_budget(&mut t, None, Some(1_000_000.0), tokens),
                TokenBudgetDecision::Continue(_)
            ));
        }
        // continuation_count = 3, delta 100 < 500 and last delta 400 < 500
        match check_token_budget(&mut t, None, Some(1_000_000.0), 1000.0) {
            TokenBudgetDecision::Stop {
                completion_event: Some(e),
            } => {
                assert_eq!(e.continuation_count, 3);
                assert!(e.diminishing_returns);
            }
            other => panic!("expected diminishing stop, got {other:?}"),
        }
    }

    #[test]
    fn large_delta_after_three_continuations_still_continues() {
        let mut t = tracker();
        for tokens in [100.0, 500.0, 900.0] {
            assert!(matches!(
                check_token_budget(&mut t, None, Some(1_000_000.0), tokens),
                TokenBudgetDecision::Continue(_)
            ));
        }
        // delta 50_000 ≥ 500 → not diminishing, below 90% → continue
        let decision = check_token_budget(&mut t, None, Some(1_000_000.0), 50_900.0);
        assert!(matches!(decision, TokenBudgetDecision::Continue(_)));
        assert_eq!(t.continuation_count, 4);
    }

    #[test]
    fn js_round_matches_javascript_semantics() {
        assert_eq!(js_round(0.5), 1);
        assert_eq!(js_round(1.5), 2);
        assert_eq!(js_round(2.4), 2);
        assert_eq!(js_round(89.5), 90);
    }
}
