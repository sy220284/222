//! Rust exports derived from original: src/hooks/useTimeout.ts
//!
//! Full equivalence of the timeout state machine: a reset trigger clears the
//! elapsed flag, and after `delay` the flag turns true. Time source is
//! injected so tests drive the clock.

#[derive(Debug, Clone, PartialEq)]
pub struct TimeoutState {
    pub delay: u64,
    is_elapsed: bool,
    scheduled_at: Option<u64>,
}

impl TimeoutState {
    /// Initial mount: not elapsed, timer scheduled at `now`.
    pub fn new(delay: u64, now: u64) -> Self {
        Self {
            delay,
            is_elapsed: false,
            scheduled_at: Some(now),
        }
    }

    /// Effect re-run on [delay, resetTrigger] change: clear + reschedule.
    pub fn reset(&mut self, delay: u64, now: u64) {
        self.delay = delay;
        self.is_elapsed = false;
        self.scheduled_at = Some(now);
    }

    /// Clock tick: fires the timer once `delay` has passed.
    pub fn tick(&mut self, now: u64) {
        if let Some(start) = self.scheduled_at {
            if now.saturating_sub(start) >= self.delay {
                self.is_elapsed = true;
            }
        }
    }

    /// `isElapsed` — the hook's return value.
    pub fn is_elapsed(&self) -> bool {
        self.is_elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_after_delay_and_resets() {
        let mut t = TimeoutState::new(500, 0);
        t.tick(499);
        assert!(!t.is_elapsed());
        t.tick(500);
        assert!(t.is_elapsed());
        t.reset(500, 1000);
        assert!(!t.is_elapsed());
        t.tick(1500);
        assert!(t.is_elapsed());
    }
}
