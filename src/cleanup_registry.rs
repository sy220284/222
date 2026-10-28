//! Explicit graceful-shutdown cleanup registration.
use anyhow::Result;
use futures::{future::join_all,Future};
use std::{collections::BTreeMap,pin::Pin,sync::{Arc,Mutex,OnceLock}};

pub type CleanupFuture=Pin<Box<dyn Future<Output=Result<()>>+Send>>;
pub type CleanupCallback=Arc<dyn Fn()->CleanupFuture+Send+Sync>;
#[derive(Default)]
struct Registry{next:u64,callbacks:BTreeMap<u64,CleanupCallback>}
fn registry()->&'static Mutex<Registry>{
    static REGISTRY:OnceLock<Mutex<Registry>>=OnceLock::new();
    REGISTRY.get_or_init(||Mutex::new(Registry::default()))
}
pub fn register_cleanup(callback:CleanupCallback)->u64{
    let mut reg=registry().lock().unwrap();
    let id=reg.next;
    reg.next+=1;
    reg.callbacks.insert(id,callback);
    id
}
pub fn unregister_cleanup(id:u64)->bool{
    registry().lock().unwrap().callbacks.remove(&id).is_some()
}
pub async fn run_cleanup_functions()->Result<()>{
    let actions=registry().lock().unwrap().callbacks.values().cloned().collect::<Vec<_>>();
    let results=join_all(actions.into_iter().map(|callback|callback())).await;
    for result in results {result?;}
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[tokio::test]
    async fn registered_async_cleanup_and_unregistration(){
        let hits=Arc::new(Mutex::new(Vec::new()));
        let first=hits.clone();
        let id1=register_cleanup(Arc::new(move||{
            let out=first.clone();
            Box::pin(async move {out.lock().unwrap().push(1);Ok(())})
        }));
        let second=hits.clone();
        let id2=register_cleanup(Arc::new(move||{
            let out=second.clone();
            Box::pin(async move {out.lock().unwrap().push(2);Ok(())})
        }));
        assert!(unregister_cleanup(id1));
        run_cleanup_functions().await.unwrap();
        assert_eq!(*hits.lock().unwrap(),vec![2]);
        assert!(unregister_cleanup(id2));
    }
}
