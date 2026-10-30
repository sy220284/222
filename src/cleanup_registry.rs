//! Explicit graceful-shutdown cleanup registration.
use anyhow::Result;
use futures::{future::join_all, Future};
use std::{
    collections::BTreeMap,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

pub type CleanupFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
pub type CleanupCallback = Arc<dyn Fn() -> CleanupFuture + Send + Sync>;
#[derive(Default)]
struct Registry {
    next: u64,
    callbacks: BTreeMap<u64, CleanupCallback>,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}
/// JavaScript cleanupRegistry uses a Set of callback identities. A repeated
/// registration of the same callback must not execute it twice or introduce a
/// second unregister target.
pub fn register_cleanup(callback: CleanupCallback) -> u64 {
    let mut reg = registry().lock().unwrap();
    if let Some((&existing_id, _)) = reg
        .callbacks
        .iter()
        .find(|(_, existing)| Arc::ptr_eq(existing, &callback))
    {
        return existing_id;
    }
    let id = reg.next;
    reg.next += 1;
    reg.callbacks.insert(id, callback);
    id
}
pub fn unregister_cleanup(id: u64) -> bool {
    registry().lock().unwrap().callbacks.remove(&id).is_some()
}
pub async fn run_cleanup_functions() -> Result<()> {
    let actions = registry()
        .lock()
        .unwrap()
        .callbacks
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let results = join_all(actions.into_iter().map(|callback| callback())).await;
    for result in results {
        result?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    // Shared global registry: keep independent async test cases isolated.
    static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    #[tokio::test]
    async fn duplicate_registration_matches_original_set_identity() {
        let _serial = TEST_LOCK.lock().await;
        let hits = Arc::new(Mutex::new(0));
        let shared = hits.clone();
        let callback: CleanupCallback = Arc::new(move || {
            let hits = shared.clone();
            Box::pin(async move {
                *hits.lock().unwrap() += 1;
                Ok(())
            })
        });
        let first = register_cleanup(callback.clone());
        let again = register_cleanup(callback);
        assert_eq!(first, again);
        run_cleanup_functions().await.unwrap();
        assert_eq!(*hits.lock().unwrap(), 1);
        assert!(unregister_cleanup(first));
        assert!(!unregister_cleanup(again));
    }
    #[tokio::test]
    async fn registered_async_cleanup_and_unregistration() {
        let _serial = TEST_LOCK.lock().await;
        let hits = Arc::new(Mutex::new(Vec::new()));
        let first = hits.clone();
        let id1 = register_cleanup(Arc::new(move || {
            let out = first.clone();
            Box::pin(async move {
                out.lock().unwrap().push(1);
                Ok(())
            })
        }));
        let second = hits.clone();
        let id2 = register_cleanup(Arc::new(move || {
            let out = second.clone();
            Box::pin(async move {
                out.lock().unwrap().push(2);
                Ok(())
            })
        }));
        assert!(unregister_cleanup(id1));
        run_cleanup_functions().await.unwrap();
        assert_eq!(*hits.lock().unwrap(), vec![2]);
        assert!(unregister_cleanup(id2));
    }
}
