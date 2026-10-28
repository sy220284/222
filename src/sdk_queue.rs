//! Local SDK bridge for the priority command queue. It intentionally does
//! not route worker-specific messages to the main Agent, nor interpret a
//! queued shell command as a model prompt.
use crate::{agent::Agent,message_queue::{Queue,Command,Priority,Mode},
    query_guard::QueryGuard};
use anyhow::{bail,Context,Result};
use serde_json::{json,Value};

pub async fn control(action:&str,request:&Value,queue:&mut Queue,
    guard:&mut QueryGuard,agent:&mut Agent)->Result<Value>{
    match action {
        "queue_enqueue"=>{
            let command=serde_json::from_value::<Command>(request.get("command")
                .cloned().context("queue_enqueue requires command")?)?;
            if !command.value.is_string() && !command.value.is_array(){
                bail!("queued value must be text or an array of content blocks");
            }
            let notification=request.get("notification").and_then(Value::as_bool).unwrap_or(false);
            let identity=queue.enqueue(command,notification);
            Ok(json!({"identity":identity,"revision":queue.revision(),"length":queue.len()}))
        }
        "queue_snapshot"=>{
            Ok(json!({"commands":queue.snapshot(),"revision":queue.revision(),
                "length":queue.len()}))
        }
        "queue_peek"=>{
            let only_main=request.get("only_main").and_then(Value::as_bool).unwrap_or(false);
            Ok(json!({"command":queue.peek(|c|!only_main||c.agent_id.is_none()).cloned(),
                "revision":queue.revision()}))
        }
        "queue_dequeue"=>{
            let only_main=request.get("only_main").and_then(Value::as_bool).unwrap_or(false);
            let item=queue.dequeue(|c|!only_main||c.agent_id.is_none());
            Ok(json!({"command":item,"revision":queue.revision(),
                "length":queue.len()}))
        }
        "queue_dequeue_all"=>{
            let items=queue.dequeue_all();
            Ok(json!({"commands":items,"revision":queue.revision(),"length":0}))
        }
        "queue_remove_identity"=>{
            let identities=request.get("identities").and_then(Value::as_array)
                .context("queue_remove_identity requires identities array")?
                .iter().map(|x|x.as_u64().context("identity must be an integer"))
                .collect::<Result<Vec<_>>>()?;
            let removed=queue.remove_identity(&identities);
            Ok(json!({"removed":removed,"revision":queue.revision(),"length":queue.len()}))
        }
        "queue_remove_agent"=>{
            let agent_id=request.get("agent_id").and_then(Value::as_str)
                .context("queue_remove_agent requires agent_id")?;
            let removed=queue.remove_matching(|c|c.agent_id.as_deref()==Some(agent_id));
            Ok(json!({"commands":removed,"revision":queue.revision(),"length":queue.len()}))
        }
        "queue_clear"=>{
            queue.clear();
            Ok(json!({"revision":queue.revision(),"length":0}))
        }
        "queue_recheck"=>{
            queue.recheck();
            Ok(json!({"revision":queue.revision(),"length":queue.len()}))
        }
        "queue_max_priority"=>{
            let priority=parse_priority(request.get("priority"))?;
            Ok(json!({"commands":queue.by_max_priority(priority),
                "revision":queue.revision()}))
        }
        "queue_pop_editable"=>{
            let text=request.get("current_input").and_then(Value::as_str).unwrap_or("");
            let cursor=request.get("cursor_offset").and_then(Value::as_u64).unwrap_or(0);
            let value=queue.pop_all_editable(text,cursor as usize);
            Ok(json!({"editable":value,"revision":queue.revision(),
                "length":queue.len()}))
        }
        "queue_drain"=>{
            let threshold=parse_priority(request.get("max_priority"))?;
            let max=request.get("max_items").and_then(Value::as_u64)
                .map(|v|v as usize).unwrap_or(usize::MAX);
            let mut results=Vec::new();
            for _ in 0..max{
                let ready=queue.peek(|c|c.agent_id.is_none() &&
                    c.priority().rank_for_sdk()<=threshold.rank_for_sdk()).cloned();
                let Some(next)=ready else {break};
                // Bail without consuming unsupported messages. A future caller
                // can edit, inspect or explicitly remove them.
                if !matches!(next.command.mode,Mode::Prompt|Mode::TaskNotification){
                    bail!("queue item {} needs {} handling; left in queue",
                        next.identity,match next.command.mode{
                            Mode::Bash=>"bash",Mode::OrphanedPermission=>"permission",
                            _=>"prompt"
                        });
                }
                if next.command.contains_image(){
                    bail!("queue item {} contains images; left in queue for multimodal transport",
                        next.identity);
                }
                let prompt=next.command.text().context("queued text unavailable")?;
                if !guard.reserve(){
                    bail!("Query already running; queue unchanged");
                }
                // Reservation is active even before any asynchronous model call.
                let item=queue.dequeue(|c|c.agent_id.is_none() &&
                    c.priority().rank_for_sdk()<=threshold.rank_for_sdk())
                    .context("queued command disappeared after reservation")?;
                let generation=guard.try_start().context("Cannot begin reserved query")?;
                let answer=agent.ask(&prompt).await;
                let ended=guard.end(generation);
                if !ended{bail!("Query generation changed during execution")}
                match answer{
                    Ok(answer)=>results.push(json!({"identity":item.identity,"ok":true,
                        "content":answer,"session_id":agent.session_id()})),
                    Err(error)=>results.push(json!({"identity":item.identity,"ok":false,
                        "error":format!("{error:#}"),"session_id":agent.session_id()})),
                }
            }
            Ok(json!({"results":results,"revision":queue.revision(),
                "length":queue.len(),"usage":agent.usage(),"cost":agent.cost_ledger()}))
        }
        _=>bail!("Unsupported queue control: {action}"),
    }
}
fn parse_priority(value:Option<&Value>)->Result<Priority>{
    if let Some(v)=value{Ok(serde_json::from_value(v.clone())
        .context("Invalid queue priority (expected now, next or later)")?)}
    else{Ok(Priority::Later)}
}
trait SdkPriority {fn rank_for_sdk(self)->u8;}
impl SdkPriority for Priority {
    fn rank_for_sdk(self)->u8{match self{Self::Now=>0,Self::Next=>1,Self::Later=>2}}
}
