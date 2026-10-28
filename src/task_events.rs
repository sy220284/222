//! Task mutations notify all subscribers in this agent process.
//! Original notifications are advisory: a subscriber failure must not
//! roll back a task mutation or prevent other subscribers from running.
use std::{
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, OnceLock},
};

type Listener=Arc<dyn Fn()+Send+Sync>;
#[derive(Default)]
struct Signal {
    next_id:u64,
    listeners:BTreeMap<u64,Listener>,
}
fn state()->&'static Mutex<Signal>{
    static SIGNAL:OnceLock<Mutex<Signal>>=OnceLock::new();
    SIGNAL.get_or_init(||Mutex::new(Signal::default()))
}
pub struct Subscription{ id:u64 }
impl Drop for Subscription {
    fn drop(&mut self){state().lock().unwrap().listeners.remove(&self.id);}
}
pub fn on_tasks_updated(listener:Listener)->Subscription {
    let mut guard=state().lock().unwrap();
    let id=guard.next_id;
    guard.next_id+=1;
    guard.listeners.insert(id,listener);
    Subscription{id}
}
pub fn notify_tasks_updated(){
    let listeners=state().lock().unwrap().listeners.values().cloned().collect::<Vec<_>>();
    for listener in listeners {
        // Catch subscriber panics. Task mutation is not an observer's concern.
        let _=catch_unwind(AssertUnwindSafe(||listener()));
    }
}
pub fn subscriber_count()->usize {state().lock().unwrap().listeners.len()}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize,Ordering};
    #[test]
    fn same_process_subscriptions_unsubscribe_and_ignore_panics(){
        let hits=Arc::new(AtomicUsize::new(0));
        let counter=hits.clone();
        let listener=on_tasks_updated(Arc::new(move||{
            counter.fetch_add(1,Ordering::SeqCst);
        }));
        let failing=on_tasks_updated(Arc::new(||panic!("observer error")));
        notify_tasks_updated();
        assert_eq!(hits.load(Ordering::SeqCst),1);
        drop(failing);
        notify_tasks_updated();
        assert_eq!(hits.load(Ordering::SeqCst),2);
        drop(listener);
    }
}
