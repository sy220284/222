//! Port of source/src/utils/signal.ts: an event-only listener set.
//! A listener is identified by Arc pointer identity, not by equality of closures.
use std::{collections::BTreeMap,sync::{Arc,Mutex,Weak}};
pub type Listener<A> = Arc<dyn Fn(A)+Send+Sync>;
struct Inner<A>{next:u64,listeners:BTreeMap<u64,Listener<A>>}
#[derive(Clone)]
pub struct Signal<A>{inner:Arc<Mutex<Inner<A>>>}
pub struct Subscription<A>{inner:Weak<Mutex<Inner<A>>>,id:u64,active:bool}
impl<A> Default for Signal<A>{fn default()->Self{Self::new()}}
impl<A> Signal<A>{
    pub fn new()->Self{Self{inner:Arc::new(Mutex::new(Inner{next:0,listeners:BTreeMap::new()}))}}
    pub fn subscribe(&self,callback:Listener<A>)->Subscription<A>{
        let mut state=self.inner.lock().unwrap();
        if let Some((&id,_))=state.listeners.iter().find(|(_,l)|Arc::ptr_eq(l,&callback)){
            return Subscription{inner:Arc::downgrade(&self.inner),id,active:true};
        }
        let id=state.next;state.next=state.next.wrapping_add(1);
        state.listeners.insert(id,callback);
        Subscription{inner:Arc::downgrade(&self.inner),id,active:true}
    }
    pub fn clear(&self){self.inner.lock().unwrap().listeners.clear()}
    pub fn len(&self)->usize{self.inner.lock().unwrap().listeners.len()}
}
impl<A:Clone> Signal<A>{
    pub fn emit(&self,args:A){
        // Unlike the JS live Set iterator, mutations during emit affect the
        // *next* emission. Keep this semantic gap in the port matrix.
        let listeners=self.inner.lock().unwrap().listeners.values().cloned().collect::<Vec<_>>();
        for listener in listeners{listener(args.clone())}
    }
}
impl<A> Subscription<A>{
    pub fn unsubscribe(&mut self){
        if !self.active{return}
        if let Some(state)=self.inner.upgrade(){state.lock().unwrap().listeners.remove(&self.id);}
        self.active=false;
    }
}
impl<A> Drop for Subscription<A>{fn drop(&mut self){self.unsubscribe();}}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn listener_order_identity_unsubscribe_and_clear(){
        let events=Arc::new(Mutex::new(Vec::new()));
        let signal=Signal::<usize>::new();
        let first=events.clone();
        let a:Listener<usize>=Arc::new(move|v|first.lock().unwrap().push(("a",v)));
        let mut a1=signal.subscribe(a.clone());
        let a2=signal.subscribe(a);
        assert_eq!(signal.len(),1);
        let second=events.clone();
        let b=signal.subscribe(Arc::new(move|v|second.lock().unwrap().push(("b",v))));
        signal.emit(1);
        assert_eq!(*events.lock().unwrap(),vec![("a",1),("b",1)]);
        a1.unsubscribe();assert_eq!(signal.len(),1);
        signal.emit(2);
        assert_eq!(*events.lock().unwrap(),vec![("a",1),("b",1),("b",2)]);
        drop(a2);drop(b);
        signal.clear();signal.emit(3);assert_eq!(signal.len(),0);
    }
}
