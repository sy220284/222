//! Rust exports derived from original: src/hooks/useElapsedTime.ts
//!
//! Duration arithmetic is fully equivalent; interval re-render subscription
//! stays with the terminal runtime (hook host pending). Formatting defers to
//! the format.ts port (p_0b4548f3115ca).

/// Raw elapsed milliseconds for the hook's `get()`:
/// `max(0, (endTime ?? now) - startTime - pausedMs)` — an explicit endTime
/// freezes the duration (viewing a 2-min task 30 min later shows "2m").
pub fn elapsed_ms(start_time: u64, now: u64, paused_ms: u64, end_time: Option<u64>) -> u64 {
    let reference = end_time.unwrap_or(now);
    reference
        .saturating_sub(start_time)
        .saturating_sub(paused_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freezes_at_end_time_and_never_negative() {
        assert_eq!(elapsed_ms(1000, 5000, 0, Some(2000)), 1000);
        assert_eq!(elapsed_ms(1000, 5000, 2000, None), 2000);
        assert_eq!(elapsed_ms(9000, 5000, 0, None), 0);
    }
}
