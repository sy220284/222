//! Source-aligned in-memory message queue; distinct from durable team mailbox.
//! Waiting consumers are notified before messages enter the queued inbox.
use crate::event_signal::{Listener,Signal,Subscription};
use serde::{Deserialize,Serialize};
use std::{collections::VecDeque,future::Future,pin::Pin,sync::{Arc,Mutex}};
use tokio::sync::oneshot;

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="lowercase")]
pub enum MessageSource{User,Teammate,System,Tick,Task}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Message{
    pub id:String,pub source:MessageSource,pub content:String,
    #[serde(skip_serializing_if="Option::is_none")] pub from:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub color:Option<String>,
    pub timestamp:String,
}
type Predicate=Arc<dyn Fn(&Message)->bool+Send+Sync>;
struct Waiter{accept:Predicate,resolve:oneshot::Sender<Message>}
struct State{queue:VecDeque<Message>,waiters:VecDeque<Waiter>,revision:u64}
#[derive(Clone)]
pub struct Mailbox{state:Arc<Mutex<State>>,signal:Signal<()>}
impl Default for Mailbox{fn default()->Self{Self::new()}}
impl Mailbox{
    pub fn new()->Self{
        Self{state:Arc::new(Mutex::new(State{
            queue:VecDeque::new(),waiters:VecDeque::new(),revision:0
        })),signal:Signal::new()}
    }
    pub fn len(&self)->usize{self.state.lock().unwrap().queue.len()}
    pub fn revision(&self)->u64{self.state.lock().unwrap().revision}
    pub fn subscribe(&self,listener:Listener<()>)->Subscription<()>{self.signal.subscribe(listener)}
    pub fn send(&self,message:Message){
        let mut state=self.state.lock().unwrap();
        state.revision=state.revision.wrapping_add(1);
        let mut undelivered=Some(message);
        while let Some(index)=state.waiters.iter().position(|w|
            (w.accept)(undelivered.as_ref().expect("pending message"))){
            let waiter=state.waiters.remove(index).expect("indexed waiter");
            match waiter.resolve.send(undelivered.take().expect("pending message")){
                Ok(())=>break,
                Err(msg)=>undelivered=Some(msg), // cancellation must not lose messages
            }
        }
        if let Some(message)=undelivered{state.queue.push_back(message);}
        drop(state);
        self.signal.emit(());
    }
    pub fn poll(&self,accept:&dyn Fn(&Message)->bool)->Option<Message>{
        let mut state=self.state.lock().unwrap();
        let index=state.queue.iter().position(accept)?;
        state.queue.remove(index)
    }
    pub fn poll_any(&self)->Option<Message>{self.poll(&|_|true)}
    // Eager registration matches JS receive(), which creates its Promise
    // and waiter when called, not when the returned future is first polled.
    pub fn receive(&self,accept:Predicate)->Pin<Box<dyn Future<Output=Message>+Send>>{
        let mut state=self.state.lock().unwrap();
        if let Some(index)=state.queue.iter().position(|msg|accept(msg)){
            let msg=state.queue.remove(index).expect("indexed message");
            drop(state);
            self.signal.emit(());
            return Box::pin(async move{msg});
        }
        let(tx,rx)=oneshot::channel();
        state.waiters.push_back(Waiter{accept,resolve:tx});
        Box::pin(async move{
            match rx.await{
                Ok(message)=>message,
                // JS promises stay pending when their mailbox is discarded;
                // dropping the owner must not turn a pending receive into panic.
                Err(_)=>std::future::pending::<Message>().await,
            }
        })
    }
    pub fn receive_any(&self)->Pin<Box<dyn Future<Output=Message>+Send>>{
        self.receive(Arc::new(|_|true))
    }
    /// Model-independent equivalent of the idle effect body in useMailboxBridge.
    pub fn bridge_when_idle<F:FnOnce(&str)->bool>(&self,is_loading:bool,submit:F)->bool{
        if is_loading{return false}
        let Some(message)=self.poll_any()else{return false};
        let _=submit(&message.content);true
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn message(id:&str,source:MessageSource)->Message{
        Message{id:id.into(),source,content:format!("content-{id}"),
            from:None,color:None,timestamp:"2026-10-29T00:00:00Z".into()}
    }
    #[test]
    fn queued_poll_does_not_notify_and_send_increments_revision(){
        let inbox=Mailbox::new();
        let hits=Arc::new(Mutex::new(0));
        let sink=hits.clone();
        let _sub=inbox.subscribe(Arc::new(move|_|*sink.lock().unwrap()+=1));
        inbox.send(message("1",MessageSource::User));
        inbox.send(message("2",MessageSource::Task));
        assert_eq!(inbox.revision(),2);assert_eq!(inbox.len(),2);
        assert_eq!(inbox.poll(&|m|m.source==MessageSource::Task).unwrap().id,"2");
        assert_eq!(inbox.len(),1);assert_eq!(*hits.lock().unwrap(),2);
        assert_eq!(inbox.poll_any().unwrap().id,"1");
        assert_eq!(inbox.revision(),2);
    }
    #[tokio::test]
    async fn pending_receivers_match_by_predicate_in_registration_order(){
        let inbox=Mailbox::new();
        let task=inbox.receive(Arc::new(|m|m.source==MessageSource::Task));
        let any=inbox.receive_any();
        inbox.send(message("human",MessageSource::User));
        assert_eq!(any.await.id,"human");
        inbox.send(message("task",MessageSource::Task));
        assert_eq!(task.await.id,"task");
        assert_eq!(inbox.len(),0);
    }
    #[tokio::test]
    async fn receiving_queued_messages_notifies_and_does_not_increment_revision(){
        let inbox=Mailbox::new();inbox.send(message("x",MessageSource::System));
        let revision=inbox.revision();
        let events=Arc::new(Mutex::new(0));
        let observed=events.clone();
        let _sub=inbox.subscribe(Arc::new(move|_|*observed.lock().unwrap()+=1));
        assert_eq!(inbox.receive_any().await.id,"x");
        assert_eq!(*events.lock().unwrap(),1);
        assert_eq!(inbox.revision(),revision);
    }
    #[tokio::test]
    async fn mailbox_disposal_keeps_unresolved_receiver_pending(){
        let pending={
            let mailbox=Mailbox::new();
            mailbox.receive_any()
        };
        let outcome=tokio::time::timeout(
            std::time::Duration::from_millis(20),pending).await;
        assert!(outcome.is_err());
    }
    #[tokio::test]
    async fn canceled_waiter_does_not_lose_future_message(){
        let inbox=Mailbox::new();
        let receiver=inbox.receive_any();drop(receiver);
        inbox.send(message("safe",MessageSource::System));
        assert_eq!(inbox.poll_any().unwrap().id,"safe");
    }
    #[test]
    fn bridge_does_not_consume_while_loading_and_ignores_submit_boolean(){
        let inbox=Mailbox::new();
        inbox.send(message("one",MessageSource::User));
        inbox.send(message("two",MessageSource::Teammate));
        assert!(!inbox.bridge_when_idle(true,|_|panic!("should not submit")));
        let mut received=Vec::new();
        assert!(inbox.bridge_when_idle(false,|s|{received.push(s.to_owned());false}));
        assert_eq!(inbox.len(),1);
        assert!(inbox.bridge_when_idle(false,|s|{received.push(s.to_owned());true}));
        assert_eq!(received,vec!["content-one","content-two"]);
        assert_eq!(inbox.revision(),2);
    }
}
