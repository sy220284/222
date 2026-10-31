//! Rust exports derived from original: src/hooks/useQueueProcessor.ts
//!
//! Queue processing gate. The unified command queue (module-level store)
//! dequeues by priority: 'now' > 'next' (user input) > 'later' (task
//! notifications). Processing triggers when no query is active (reactive
//! guard), the queue has items, and no local JSX UI blocks input.

/// Whether the dequeue pump may run this tick.
pub fn should_process_queue(
    query_active: bool,
    queue_len: usize,
    local_jsx_ui_active: bool,
) -> bool {
    !query_active && queue_len > 0 && !local_jsx_ui_active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_semantics() {
        assert!(should_process_queue(false, 1, false));
        assert!(!should_process_queue(true, 1, false));
        assert!(!should_process_queue(false, 0, false));
        assert!(!should_process_queue(false, 1, true));
    }
}
