//! Workspace-local, provider-neutral task-list storage and task tool semantics.
//! Source references: utils/tasks.ts, TaskCreate/Get/List/UpdateTool.ts.
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize,Serialize};
use serde_json::{json,Map,Value};
use std::{collections::HashSet,fs,fs::OpenOptions,path::{Path,PathBuf}};
use uuid::Uuid;
use crate::protocol::ToolCall;

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase")]
pub struct Item {
    pub id:String,
    pub subject:String,
    pub description:String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub active_form:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub owner:Option<String>,
    pub status:String,
    #[serde(default)]
    pub blocks:Vec<String>,
    #[serde(default)]
    pub blocked_by:Vec<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub metadata:Option<Map<String,Value>>,
}

#[derive(Clone)]
pub struct TaskGraph { directory:PathBuf }
impl TaskGraph {
    pub fn new(root:&Path,list_id:&str)->Result<Self>{
        if list_id.is_empty() || list_id.len()>128 {bail!("Invalid task list ID")}
        let safe=list_id.chars().map(|c|if c.is_ascii_alphanumeric()||c=='_'||c=='-' {c}else{'-'}).collect::<String>();
        let root=root.canonicalize().context("Workspace unavailable")?;
        let base=root.join(".atlas").join("task-lists");
        fs::create_dir_all(&base)?;
        let base=base.canonicalize()?;
        if !base.starts_with(&root){bail!("Task storage directory escapes workspace")}
        let path=base.join(safe);
        fs::create_dir_all(&path)?;
        let directory=path.canonicalize()?;
        if !directory.starts_with(&base){bail!("Task list escapes workspace")}
        Ok(Self{directory})
    }
    fn locked<F,T>(&self, action:F)->Result<T>
    where F:FnOnce(&Self)->Result<T>{
        let path=self.directory.join(".lock");
        if path.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()){bail!("Unsafe task lock link")}
        let lock=OpenOptions::new().write(true).create(true).open(path)?;
        lock.lock_exclusive().context("Task list lock failed")?;
        let result=action(self);
        let _=FileExt::unlock(&lock);
        result
    }
    fn path(&self,id:&str)->Result<PathBuf>{
        if id.is_empty() || !id.bytes().all(|c|c.is_ascii_digit()) || id.len()>20 {bail!("Invalid task ID")}
        let path=self.directory.join(format!("{id}.json"));
        if path.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()){bail!("Unsafe task file link")}
        Ok(path)
    }
    fn load(&self,id:&str)->Result<Option<Item>>{
        let path=self.path(id)?;
        let bytes=match fs::read(path) {Ok(v)=>v,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(e)=>return Err(e.into())};
        if bytes.len()>1_000_000 {bail!("Task record too large")}
        let item:Item=serde_json::from_slice(&bytes)?;
        if item.id!=id || !matches!(item.status.as_str(),"pending"|"in_progress"|"completed"){bail!("Invalid task record")}
        Ok(Some(item))
    }
    fn save(&self,item:&Item)->Result<()>{
        let path=self.path(&item.id)?;
        let temp=self.directory.join(format!(".{}.tmp",Uuid::new_v4()));
        let data=serde_json::to_vec_pretty(item)?;
        if data.len()>1_000_000 {bail!("Task record too large")}
        let mut file=OpenOptions::new().write(true).create_new(true).open(&temp)?;
        use std::io::Write;
        let result=(||{file.write_all(&data)?;file.sync_all()?;fs::rename(&temp,&path)?;Ok::<_,anyhow::Error>(())})();
        if result.is_err(){let _=fs::remove_file(&temp);}
        result
    }
    fn all(&self)->Result<Vec<Item>>{
        let mut tasks=Vec::new();
        for ent in fs::read_dir(&self.directory)?{
            let ent=ent?;
            if !ent.file_type()?.is_file(){continue}
            let path=ent.path();
            if path.extension().and_then(|s|s.to_str())!=Some("json"){continue}
            let Some(name)=path.file_stem().and_then(|s|s.to_str())else{continue};
            if !name.bytes().all(|c|c.is_ascii_digit()){continue}
            if let Some(item)=self.load(name)?{tasks.push(item)}
        }
        tasks.sort_by(|a,b|a.id.parse::<u64>().unwrap_or(0).cmp(&b.id.parse::<u64>().unwrap_or(0)));
        Ok(tasks)
    }
    fn highest(&self)->Result<u64>{
        let mut high=fs::read_to_string(self.directory.join(".highwatermark")).ok()
            .and_then(|s|s.trim().parse::<u64>().ok()).unwrap_or(0);
        for task in self.all()? {high=high.max(task.id.parse().unwrap_or(0));}
        Ok(high)
    }
    fn mark(&self,id:u64)->Result<()>{
        fs::write(self.directory.join(".highwatermark"),id.to_string())?;
        Ok(())
    }
    fn required<'a>(args:&'a Value,key:&str)->Result<&'a str>{
        args.get(key).and_then(Value::as_str).with_context(||format!("Missing {key}"))
    }
    fn optional_string(args:&Value,key:&str)->Result<Option<String>>{
        match args.get(key){None=>Ok(None),Some(Value::String(s))=>Ok(Some(s.clone())),_=>bail!("Invalid {key}")}
    }
    fn strict(args:&Value,allowed:&[&str])->Result<()>{
        let map=args.as_object().context("Task parameters must be an object")?;
        if let Some(key)=map.keys().find(|k|!allowed.contains(&k.as_str())){bail!("Unrecognized task parameter {key}")}
        Ok(())
    }
    fn ids(args:&Value,key:&str)->Result<Vec<String>>{
        match args.get(key) {
            None=>Ok(vec![]),
            Some(Value::Array(values))=>values.iter().map(|v|{
                let id=v.as_str().context("Task IDs must be strings")?;
                if id.is_empty()||!id.bytes().all(|b|b.is_ascii_digit()){bail!("Invalid dependency ID")}
                Ok(id.to_owned())
            }).collect(),
            _=>bail!("{key} must be an array of IDs")
        }
    }
    pub fn create(&self,args:&Value)->Result<String>{
        Self::strict(args,&["subject","description","activeForm","metadata"])?;
        let subject=Self::required(args,"subject")?;
        let description=Self::required(args,"description")?;
        let active=Self::optional_string(args,"activeForm")?;
        let metadata=match args.get("metadata"){None=>None,Some(Value::Object(m))=>Some(m.clone()),_=>bail!("metadata must be an object")};
        if subject.len()>2000||description.len()>200_000 {bail!("Task input too large")}
        self.locked(|this|{
            let id=this.highest()?.checked_add(1).context("Task counter exhausted")?;
            let task=Item{id:id.to_string(),subject:subject.into(),description:description.into(),
                active_form:active,owner:None,status:"pending".into(),blocks:vec![],blocked_by:vec![],metadata};
            this.save(&task)?;
            this.mark(id)?;
            Ok(format!("Task #{} created successfully: {}",task.id,task.subject))
        })
    }
    pub fn peek(&self,id:&str)->Result<Option<Item>>{self.load(id)}
    pub fn delete_for_blocking_hook(&self,id:&str)->Result<bool>{
        self.locked(|this|this.delete(id))
    }
    pub fn get(&self,args:&Value)->Result<String>{
        Self::strict(args,&["taskId"])?;
        let id=Self::required(args,"taskId")?;
        match self.load(id)?{
            None=>Ok("Task not found".into()),
            Some(task)=>{
                let mut text=format!("Task #{}: {}\nStatus: {}\nDescription: {}",
                    task.id,task.subject,task.status,task.description);
                if !task.blocked_by.is_empty(){text.push_str(&format!("\nBlocked by: {}",task.blocked_by.iter().map(|x|format!("#{x}")).collect::<Vec<_>>().join(", ")));}
                if !task.blocks.is_empty(){text.push_str(&format!("\nBlocks: {}",task.blocks.iter().map(|x|format!("#{x}")).collect::<Vec<_>>().join(", ")));}
                Ok(text)
            }
        }
    }
    pub fn list(&self,args:&Value)->Result<String>{
        Self::strict(args,&[])?;
        let tasks=self.all()?.into_iter().filter(|t|!t.metadata.as_ref()
            .and_then(|m|m.get("_internal")).and_then(Value::as_bool).unwrap_or(false)).collect::<Vec<_>>();
        if tasks.is_empty(){return Ok("No tasks found".into())}
        let completed=tasks.iter().filter(|t|t.status=="completed").map(|t|t.id.clone()).collect::<HashSet<_>>();
        Ok(tasks.iter().map(|task|{
            let owner=task.owner.as_ref().map(|x|format!(" ({x})")).unwrap_or_default();
            let blockers=task.blocked_by.iter().filter(|id|!completed.contains(*id))
                .map(|id|format!("#{id}")).collect::<Vec<_>>();
            let blocked=if blockers.is_empty(){String::new()}else{format!(" [blocked by {}]",blockers.join(", "))};
            format!("#{} [{}] {}{owner}{blocked}",task.id,task.status,task.subject)
        }).collect::<Vec<_>>().join("\n"))
    }
    fn link(&self,from:&str,to:&str)->Result<bool>{
        let (Some(mut source),Some(mut target))=(self.load(from)?,self.load(to)?)else{return Ok(false)};
        if !source.blocks.contains(&to.to_owned()){
            source.blocks.push(to.to_owned());self.save(&source)?;
        }
        if !target.blocked_by.contains(&from.to_owned()){
            target.blocked_by.push(from.to_owned());self.save(&target)?;
        }
        Ok(true)
    }
    fn delete(&self,id:&str)->Result<bool>{
        let path=self.path(id)?;
        if self.load(id)?.is_none(){return Ok(false)}
        let num=id.parse::<u64>()?;
        self.mark(self.highest()?.max(num))?;
        fs::remove_file(path)?;
        for mut task in self.all()?{
            let a=task.blocks.len();let b=task.blocked_by.len();
            task.blocks.retain(|x|x!=id);task.blocked_by.retain(|x|x!=id);
            if a!=task.blocks.len()||b!=task.blocked_by.len(){self.save(&task)?}
        }
        Ok(true)
    }
    pub fn update(&self,args:&Value)->Result<String>{
        Self::strict(args,&["taskId","subject","description","activeForm","status","owner","addBlocks","addBlockedBy","metadata"])?;
        let id=Self::required(args,"taskId")?;
        let subject=Self::optional_string(args,"subject")?;
        let description=Self::optional_string(args,"description")?;
        let active=Self::optional_string(args,"activeForm")?;
        let owner=Self::optional_string(args,"owner")?;
        let status=Self::optional_string(args,"status")?;
        let blocks=Self::ids(args,"addBlocks")?;
        let blocked_by=Self::ids(args,"addBlockedBy")?;
        let meta=match args.get("metadata"){None=>None,Some(Value::Object(m))=>Some(m.clone()),_=>bail!("metadata must be an object")};
        if let Some(state)=&status{
            if !matches!(state.as_str(),"pending"|"in_progress"|"completed"|"deleted"){bail!("Invalid task status")}
        }
        self.locked(|this|{
            let Some(mut task)=this.load(id)?else{return Ok("Task not found".into())};
            if status.as_deref()==Some("deleted"){
                return if this.delete(id)?{Ok(format!("Updated task #{id} deleted"))}else{Ok("Failed to delete task".into())}
            }
            let mut fields=Vec::new();
            if let Some(s)=&subject{if *s!=task.subject{task.subject=s.clone();fields.push("subject")}}
            if let Some(s)=&description{if *s!=task.description{task.description=s.clone();fields.push("description")}}
            if let Some(s)=&active{if task.active_form.as_ref()!=Some(s){task.active_form=Some(s.clone());fields.push("activeForm")}}
            if let Some(s)=&owner{if task.owner.as_ref()!=Some(s){task.owner=Some(s.clone());fields.push("owner")}}
            if let Some(s)=&status{if *s!=task.status{task.status=s.clone();fields.push("status")}}
            if let Some(patch)=meta{
                let m=task.metadata.get_or_insert_with(Map::new);
                for (key,value) in patch{if value.is_null(){m.remove(&key);}else{m.insert(key,value);}}
                fields.push("metadata");
            }
            if !fields.is_empty(){this.save(&task)?}
            if !blocks.is_empty(){
                let existing=task.blocks.clone();
                let new=blocks.iter().filter(|x|!existing.contains(*x)).collect::<Vec<_>>();
                for other in &new{this.link(id,other)?;}
                if !new.is_empty(){fields.push("blocks")}
            }
            if !blocked_by.is_empty(){
                let existing=task.blocked_by.clone();
                let new=blocked_by.iter().filter(|x|!existing.contains(*x)).collect::<Vec<_>>();
                for other in &new{this.link(other,id)?;}
                if !new.is_empty(){fields.push("blockedBy")}
            }
            Ok(format!("Updated task #{id} {}",fields.join(", ")).trim_end().to_owned())
        })
    }

    pub fn reset(&self)->Result<()>{
        self.locked(|store|{
            let high=store.highest()?;
            for item in store.all()? { fs::remove_file(store.path(&item.id)?)?; }
            store.mark(high)
        })
    }

    pub fn member_statuses(&self,members:&[(String,String,Option<String>)])->Result<Vec<Value>>{
        let tasks=self.all()?;
        Ok(members.iter().map(|(id,name,kind)|{
            let owned=tasks.iter().filter(|t|
                t.status!="completed" &&
                (t.owner.as_deref()==Some(name)||t.owner.as_deref()==Some(id))
            ).map(|t|t.id.clone()).collect::<Vec<_>>();
            json!({"agentId":id,"name":name,"agentType":kind,
                "status":if owned.is_empty(){"idle"}else{"busy"},"currentTasks":owned})
        }).collect())
    }

    pub fn unassign_teammate(&self,agent_id:&str,name:&str,reason:&str)->Result<Value>{
        if !matches!(reason,"terminated"|"shutdown"){bail!("Invalid exit reason")}
        self.locked(|store|{
            let mut removed=Vec::<Value>::new();
            for mut item in store.all()?{
                if item.status=="completed" ||
                    (item.owner.as_deref()!=Some(agent_id)&&item.owner.as_deref()!=Some(name)){
                    continue;
                }
                removed.push(json!({"id":item.id,"subject":item.subject}));
                item.owner=None;
                item.status="pending".into();
                store.save(&item)?;
            }
            let verb=if reason=="terminated"{"was terminated"}else{"has shut down"};
            let mut message=format!("{name} {verb}.");
            if !removed.is_empty(){
                let listing=removed.iter().map(|v|
                    format!("#{} \"{}\"",v["id"].as_str().unwrap_or(""),v["subject"].as_str().unwrap_or(""))
                ).collect::<Vec<_>>().join(", ");
                message.push_str(&format!(" {} task(s) were unassigned: {listing}. Use TaskList to check availability and TaskUpdate with owner to reassign them to idle teammates.",removed.len()));
            }
            Ok(json!({"unassignedTasks":removed,"notificationMessage":message}))
        })
    }

    pub fn claim(&self,id:&str,owner:&str,check_busy:bool)->Result<Value>{
        self.locked(|this|{
            let Some(mut task)=this.load(id)?else{return Ok(json!({"success":false,"reason":"task_not_found"}))};
            if task.owner.as_deref().is_some_and(|x|x!=owner){return Ok(json!({"success":false,"reason":"already_claimed","task":task}))}
            if task.status=="completed"{return Ok(json!({"success":false,"reason":"already_resolved","task":task}))}
            let all=this.all()?;
            let blockers=task.blocked_by.iter().filter(|b|all.iter().any(|t|t.id==**b&&t.status!="completed")).cloned().collect::<Vec<_>>();
            if !blockers.is_empty(){return Ok(json!({"success":false,"reason":"blocked","blockedByTasks":blockers,"task":task}))}
            let busy=all.iter().filter(|t|t.owner.as_deref()==Some(owner)&&t.status!="completed"&&t.id!=id)
                .map(|t|t.id.clone()).collect::<Vec<_>>();
            if check_busy&&!busy.is_empty(){return Ok(json!({"success":false,"reason":"agent_busy","busyWithTasks":busy,"task":task}))}
            task.owner=Some(owner.to_owned());this.save(&task)?;
            Ok(json!({"success":true,"task":task}))
        })
    }
    pub fn run(&self,call:&ToolCall)->String{
        let parsed=serde_json::from_str::<Value>(&call.function.arguments);
        let result=parsed.and_then(|v|Ok(v)).map_err(anyhow::Error::from).and_then(|v|{
            match call.function.name.as_str(){
                "TaskCreate"=>self.create(&v),"TaskGet"=>self.get(&v),
                "TaskList"=>self.list(&v),"TaskUpdate"=>self.update(&v),
                _=>bail!("Unknown task action")
            }
        });
        match result{Ok(s)=>s,Err(e)=>format!("ERROR: {e:#}")}
    }
}
pub fn handles(name:&str)->bool{matches!(name,"TaskCreate"|"TaskGet"|"TaskList"|"TaskUpdate")}
pub fn definitions()->Vec<Value>{
    vec![
        json!({"type":"function","function":{"name":"TaskCreate","description":"Create a persistent task with subject and description.","parameters":{"type":"object","additionalProperties":false,"properties":{"subject":{"type":"string"},"description":{"type":"string"},"activeForm":{"type":"string"},"metadata":{"type":"object"}},"required":["subject","description"]}}}),
        json!({"type":"function","function":{"name":"TaskGet","description":"Read a persistent task by ID.","parameters":{"type":"object","additionalProperties":false,"properties":{"taskId":{"type":"string"}},"required":["taskId"]}}}),
        json!({"type":"function","function":{"name":"TaskList","description":"List persistent tasks, removing completed dependency blockers.","parameters":{"type":"object","additionalProperties":false,"properties":{}}}}),
        json!({"type":"function","function":{"name":"TaskUpdate","description":"Update task fields, owner, status, dependencies or metadata; delete with status deleted.","parameters":{"type":"object","additionalProperties":false,"properties":{"taskId":{"type":"string"},"subject":{"type":"string"},"description":{"type":"string"},"activeForm":{"type":"string"},"status":{"type":"string","enum":["pending","in_progress","completed","deleted"]},"owner":{"type":"string"},"addBlocks":{"type":"array","items":{"type":"string"}},"addBlockedBy":{"type":"array","items":{"type":"string"}},"metadata":{"type":"object"}},"required":["taskId"]}}}),
    ]
}
#[cfg(test)]
mod tests{
    use super::*;
    fn store()->(tempfile::TempDir,TaskGraph){
        let root=tempfile::tempdir().unwrap();
        let graph=TaskGraph::new(root.path(),"session-1").unwrap();
        (root,graph)
    }
    #[test]
    fn full_task_lifecycle_dependency_owner_metadata_and_reload(){
        let (root,g)=store();
        assert_eq!(g.create(&json!({"subject":"Build","description":"Build system"})).unwrap(),"Task #1 created successfully: Build");
        assert_eq!(g.create(&json!({"subject":"Verify","description":"Run tests","activeForm":"Testing"})).unwrap(),"Task #2 created successfully: Verify");
        assert!(g.update(&json!({"taskId":"2","addBlockedBy":["1"],"owner":"agent-A","metadata":{"revision":1,"remove":true}})).unwrap().contains("blockedBy"));
        assert!(g.get(&json!({"taskId":"1"})).unwrap().contains("Blocks: #2"));
        assert!(g.list(&json!({})).unwrap().contains("[blocked by #1]"));
        let not_claimed=g.claim("2","agent-B",false).unwrap();
        assert_eq!(not_claimed["reason"],"already_claimed");
        let blocked=g.claim("2","agent-A",false).unwrap();
        assert_eq!(blocked["reason"],"blocked");
        assert!(g.update(&json!({"taskId":"1","status":"completed"})).unwrap().contains("status"));
        assert!(!g.list(&json!({})).unwrap().contains("[blocked by #1]"));
        let claimed=g.claim("2","agent-A",true).unwrap();
        assert_eq!(claimed["success"],true);
        assert!(g.update(&json!({"taskId":"2","metadata":{"remove":null,"revision":2},"status":"in_progress"})).unwrap().contains("metadata"));
        let task=g.load("2").unwrap().unwrap();
        assert_eq!(task.metadata.unwrap().get("revision"),Some(&json!(2)));
        let fresh=TaskGraph::new(root.path(),"session-1").unwrap();
        assert_eq!(fresh.all().unwrap().len(),2);
        assert!(fresh.update(&json!({"taskId":"1","status":"deleted"})).unwrap().contains("deleted"));
        assert!(fresh.get(&json!({"taskId":"2"})).unwrap().contains("Task #2"));
        assert!(!fresh.get(&json!({"taskId":"2"})).unwrap().contains("Blocked by"));
        assert!(fresh.create(&json!({"subject":"Followup","description":"Next"})).unwrap().contains("#3"));
    }
    #[test]
    fn concurrent_creators_get_unique_monotonic_ids(){
        let (root,_g)=store();
        let root=std::sync::Arc::new(root);
        let workers=(0..12).map(|i|{
            let root=root.clone();
            std::thread::spawn(move||{
                let graph=TaskGraph::new(root.path(),"session-1").unwrap();
                graph.create(&json!({"subject":format!("Worker {i}"),"description":"Parallel"})).unwrap()
            })
        }).collect::<Vec<_>>();
        let mut ids=workers.into_iter().map(|t|t.join().unwrap().split_whitespace()
            .nth(1).unwrap().trim_start_matches('#').parse::<u64>().unwrap())
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids,(1..=12).collect::<Vec<_>>());
        let graph=TaskGraph::new(root.path(),"session-1").unwrap();
        assert_eq!(graph.all().unwrap().len(),12);
    }

    #[test]
    fn validates_input_and_rejects_path_traversal(){
        let (root,g)=store();
        assert!(g.create(&json!({"subject":"x"})).is_err());
        assert!(g.create(&json!({"subject":"x","description":"ok","unknown":true})).is_err());
        assert!(g.get(&json!({"taskId":"../outside"})).is_err());
        assert!(TaskGraph::new(root.path(),"../n").is_ok());
        assert!(g.update(&json!({"taskId":"1","status":"blocked"})).is_err());
        assert_eq!(g.get(&json!({"taskId":"999"})).unwrap(),"Task not found");
    }
    #[test]
    fn internal_tasks_hidden_and_busy_claim_fails(){
        let (_root,g)=store();
        g.create(&json!({"subject":"A","description":"desc"})).unwrap();
        g.create(&json!({"subject":"B","description":"desc","metadata":{"_internal":true}})).unwrap();
        g.create(&json!({"subject":"C","description":"desc"})).unwrap();
        assert_eq!(g.list(&json!({})).unwrap().lines().count(),2);
        assert_eq!(g.claim("1","agent",true).unwrap()["success"],true);
        assert_eq!(g.claim("3","agent",true).unwrap()["reason"],"agent_busy");
    }
}
