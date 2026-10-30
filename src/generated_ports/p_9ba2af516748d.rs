//! Rust exports derived from original: src/hooks/useCommandQueue.ts
//!
//! Subscription surface for the unified command queue. The original wires
//! React's useSyncExternalStore to messageQueueManager's two accessors
//! (frozen snapshot that changes reference only on mutation, plus a
//! subscribe callback). This crate's store lives in `message_queue`
//! (snapshot()/subscribe() on the queue type); the hook adds no semantics
//! beyond that wiring, so the port records the binding instead of
//! duplicating the store.

/// The unified command queue store behind the original hook:
/// `crate::message_queue` (snapshot()/subscribe() on the queue type).
pub const COMMAND_QUEUE_STORE_PATH: &str = "crate::message_queue";
