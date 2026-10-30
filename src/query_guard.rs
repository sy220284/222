//! Query lifecycle guard from src/utils/QueryGuard.ts.
//! In particular, the dequeued->dispatching async gap is marked as active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Dispatching,
    Running,
}
pub struct QueryGuard {
    status: Status,
    generation: u64,
    listeners: Vec<(u64, Box<dyn Fn()>)>,
    next_listener: u64,
}
impl Default for QueryGuard {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            generation: 0,
            listeners: Vec::new(),
            next_listener: 0,
        }
    }
}
impl QueryGuard {
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn is_active(&self) -> bool {
        self.status != Status::Idle
    }
    pub fn snapshot(&self) -> bool {
        self.is_active()
    }
    fn notify(&self) {
        for (_, listener) in &self.listeners {
            listener()
        }
    }
    pub fn subscribe(&mut self, callback: impl Fn() + 'static) -> u64 {
        self.next_listener += 1;
        let id = self.next_listener;
        self.listeners.push((id, Box::new(callback)));
        id
    }
    pub fn unsubscribe(&mut self, id: u64) {
        self.listeners.retain(|(existing, _)| *existing != id);
    }
    pub fn reserve(&mut self) -> bool {
        if self.status != Status::Idle {
            return false;
        }
        self.status = Status::Dispatching;
        self.notify();
        true
    }
    pub fn cancel_reservation(&mut self) {
        if self.status != Status::Dispatching {
            return;
        }
        self.status = Status::Idle;
        self.notify();
    }
    pub fn try_start(&mut self) -> Option<u64> {
        if self.status == Status::Running {
            return None;
        }
        self.status = Status::Running;
        self.generation += 1;
        self.notify();
        Some(self.generation)
    }
    pub fn end(&mut self, generation: u64) -> bool {
        if self.status != Status::Running || generation != self.generation {
            return false;
        }
        self.status = Status::Idle;
        self.notify();
        true
    }
    pub fn force_end(&mut self) {
        if self.status == Status::Idle {
            return;
        }
        self.status = Status::Idle;
        self.generation += 1;
        self.notify();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    #[test]
    fn lifecycle_direct_and_reserved_queries_prevent_races() {
        let mut guard = QueryGuard::default();
        assert_eq!(guard.status(), Status::Idle);
        assert!(guard.reserve());
        assert!(!guard.reserve());
        assert!(guard.snapshot());
        let generation = guard.try_start().unwrap();
        assert_eq!(generation, 1);
        assert_eq!(guard.try_start(), None);
        assert!(!guard.end(generation + 1));
        assert!(guard.end(generation));
        assert!(!guard.end(generation));
        assert!(!guard.is_active());
        assert_eq!(guard.try_start(), Some(2));
        assert!(guard.end(2));
    }
    #[test]
    fn cancelling_or_forcing_end_invalidates_stale_finally() {
        let mut guard = QueryGuard::default();
        guard.reserve();
        guard.cancel_reservation();
        assert!(!guard.is_active());
        guard.cancel_reservation();
        guard.reserve();
        let stale = guard.try_start().unwrap();
        guard.force_end();
        assert!(!guard.end(stale));
        assert_eq!(guard.try_start(), Some(stale + 2));
        assert!(guard.end(stale + 2));
        guard.force_end();
        assert_eq!(guard.generation(), stale + 2);
    }
    #[test]
    fn notifies_subscribers_synchronously_only_on_real_transitions() {
        let mut guard = QueryGuard::default();
        let count = Rc::new(Cell::new(0));
        let shared = count.clone();
        let subscription = guard.subscribe(move || shared.set(shared.get() + 1));
        assert!(guard.reserve());
        assert!(!guard.reserve());
        guard.cancel_reservation();
        let generation = guard.try_start().unwrap();
        guard.end(generation);
        assert_eq!(count.get(), 4);
        guard.unsubscribe(subscription);
        guard.reserve();
        guard.cancel_reservation();
        assert_eq!(count.get(), 4);
    }
}
