//! Task mutations notify all subscribers in this agent process.
//! Original notifications are advisory: a subscriber failure must not
//! roll back a task mutation or prevent other subscribers from running.
use std::{
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, OnceLock},
};

type Listener = Arc<dyn Fn() + Send + Sync>;
#[derive(Default)]
struct Signal {
    next_id: u64,
    listeners: BTreeMap<u64, Listener>,
}
fn state() -> &'static Mutex<Signal> {
    static SIGNAL: OnceLock<Mutex<Signal>> = OnceLock::new();
    SIGNAL.get_or_init(|| Mutex::new(Signal::default()))
}
thread_local! {
    static DEFERRED:std::cell::RefCell<(usize,usize)>=
        const {std::cell::RefCell::new((0,0))};
}
pub struct BatchGuard;
pub fn defer_notifications() -> BatchGuard {
    DEFERRED.with(|state| state.borrow_mut().0 += 1);
    BatchGuard
}
impl Drop for BatchGuard {
    fn drop(&mut self) {
        let pending = DEFERRED.with(|state| {
            let mut state = state.borrow_mut();
            state.0 -= 1;
            if state.0 == 0 {
                std::mem::take(&mut state.1)
            } else {
                0
            }
        });
        for _ in 0..pending {
            dispatch()
        }
    }
}
pub struct Subscription {
    id: u64,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        state().lock().unwrap().listeners.remove(&self.id);
    }
}
pub fn on_tasks_updated(listener: Listener) -> Subscription {
    let mut guard = state().lock().unwrap();
    let id = guard.next_id;
    guard.next_id += 1;
    guard.listeners.insert(id, listener);
    Subscription { id }
}
pub fn notify_tasks_updated() {
    let deferred = DEFERRED.with(|state| {
        let mut state = state.borrow_mut();
        if state.0 > 0 {
            state.1 += 1;
            true
        } else {
            false
        }
    });
    if !deferred {
        dispatch()
    }
}
fn dispatch() {
    let listeners = state()
        .lock()
        .unwrap()
        .listeners
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for listener in listeners {
        let _ = catch_unwind(AssertUnwindSafe(|| listener()));
    }
}
pub fn subscriber_count() -> usize {
    state().lock().unwrap().listeners.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn same_process_subscriptions_unsubscribe_and_ignore_panics() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let listener = on_tasks_updated(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        let failing = on_tasks_updated(Arc::new(|| panic!("observer error")));
        // The observer bus is process-wide. Other parallel tests may publish
        // notifications, so verify changes relative to each subscription's
        // current count instead of assuming an isolated global event total.
        let before = hits.load(Ordering::SeqCst);
        notify_tasks_updated();
        assert!(hits.load(Ordering::SeqCst) > before);
        drop(failing);
        let before = hits.load(Ordering::SeqCst);
        notify_tasks_updated();
        assert!(hits.load(Ordering::SeqCst) > before);
        drop(listener);
        let after_drop = hits.load(Ordering::SeqCst);
        notify_tasks_updated();
        assert_eq!(hits.load(Ordering::SeqCst), after_drop);
    }
}
