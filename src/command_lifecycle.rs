//! A single globally configured listener for command start/finish events.
use std::sync::{Arc,Mutex,OnceLock};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum State{Started,Completed}
pub type Listener=Arc<dyn Fn(&str,State)+Send+Sync>;
fn slot()->&'static Mutex<Option<Listener>>{
    static LISTENER:OnceLock<Mutex<Option<Listener>>>=OnceLock::new();
    LISTENER.get_or_init(||Mutex::new(None))
}
pub fn set_command_lifecycle_listener(callback:Option<Listener>){
    *slot().lock().unwrap()=callback;
}
pub fn notify_command_lifecycle(id:&str,state:State){
    let callback=slot().lock().unwrap().clone();
    if let Some(f)=callback{f(id,state);}
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn single_listener_replace_clear_and_event_order(){
        let observed=Arc::new(Mutex::new(Vec::new()));
        let sink=observed.clone();
        set_command_lifecycle_listener(Some(Arc::new(move |id,state|{
            sink.lock().unwrap().push((id.to_owned(),state));
        })));
        notify_command_lifecycle("a",State::Started);
        notify_command_lifecycle("a",State::Completed);
        assert_eq!(*observed.lock().unwrap(),vec![
            ("a".to_owned(),State::Started),("a".to_owned(),State::Completed)]);
        set_command_lifecycle_listener(None);
        notify_command_lifecycle("b",State::Started);
        assert_eq!(observed.lock().unwrap().len(),2);
    }
}
