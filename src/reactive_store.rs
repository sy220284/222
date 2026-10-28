//! Single-value store with subscriptions and change callbacks.
use std::{collections::BTreeMap,sync::{Arc,Mutex,Weak}};

type Listener=Arc<dyn Fn()+Send+Sync>;
type OnChange<T>=Arc<dyn Fn(T,T)+Send+Sync>;
struct Inner<T>{
    state:T,
    listeners:BTreeMap<usize,Listener>,
    next:usize,
    on_change:Option<OnChange<T>>,
}
#[derive(Clone)]
pub struct Store<T>{inner:Arc<Mutex<Inner<T>>>}
pub struct Subscription<T>{inner:Weak<Mutex<Inner<T>>>,id:usize}
impl<T> Drop for Subscription<T>{
    fn drop(&mut self){
        if let Some(inner)=self.inner.upgrade(){
            inner.lock().unwrap().listeners.remove(&self.id);
        }
    }
}
impl<T:Clone+PartialEq> Store<T>{
    pub fn create(initial:T)->Self{Self::with_on_change(initial,None)}
    pub fn with_on_change(initial:T,on_change:Option<OnChange<T>>)->Self{
        Self{inner:Arc::new(Mutex::new(Inner{
            state:initial,listeners:BTreeMap::new(),next:0,on_change
        }))}
    }
    pub fn get_state(&self)->T{self.inner.lock().unwrap().state.clone()}
    pub fn set_state<F:FnOnce(&T)->T>(&self,updater:F){
        let callbacks={
            let mut inner=self.inner.lock().unwrap();
            let previous=inner.state.clone();
            let next=updater(&previous);
            if next==previous{return;}
            inner.state=next.clone();
            (inner.on_change.clone(),previous,next,
                inner.listeners.values().cloned().collect::<Vec<_>>())
        };
        let (on_change,old,new,listeners)=callbacks;
        if let Some(callback)=on_change{callback(new,old);}
        for listener in listeners{listener();}
    }
    pub fn subscribe(&self,listener:Listener)->Subscription<T>{
        let mut inner=self.inner.lock().unwrap();
        let id=inner.next;
        inner.next+=1;
        inner.listeners.insert(id,listener);
        Subscription{inner:Arc::downgrade(&self.inner),id}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn subscription_change_suppression_order_and_cleanup(){
        let history=Arc::new(Mutex::new(Vec::new()));
        let events=history.clone();
        let store=Store::with_on_change(false,Some(Arc::new(move|new,old|{
            events.lock().unwrap().push(format!("change:{old}->{new}"));
        })));
        let events=history.clone();
        let subscription=store.subscribe(Arc::new(move||{
            events.lock().unwrap().push("listener".to_string());
        }));
        assert!(!store.get_state());
        store.set_state(|v|*v);
        assert!(history.lock().unwrap().is_empty());
        store.set_state(|_|true);
        assert!(store.get_state());
        assert_eq!(*history.lock().unwrap(),vec!["change:false->true","listener"]);
        drop(subscription);
        store.set_state(|_|false);
        assert_eq!(history.lock().unwrap().len(),3);
    }
}
