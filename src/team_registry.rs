use anyhow::{bail,Context,Result};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use std::{fs,fs::OpenOptions,io::Write,path::{Path,PathBuf}};
use fs2::FileExt;
use crate::team_mailbox::{Mailbox,Message};

#[derive(Clone,Serialize,Deserialize)]
pub struct Member {pub name:String,pub id:String,pub active:bool}
#[derive(Clone,Serialize,Deserialize)]
pub struct Team {
    pub name:String,pub description:Option<String>,
    pub lead_session:String,pub lead_id:String,pub members:Vec<Member>,
}
pub struct Teams {root:PathBuf}
impl Teams {
    pub fn new(root:&Path)->Result<Self>{
        let root=root.canonicalize()?;
        let teams=root.join(".atlas/teams");
        fs::create_dir_all(&teams)?;
        if !teams.canonicalize()?.starts_with(&root){bail!("Teams escape workspace")}
        Ok(Self{root})
    }
    fn locked<T,F:FnOnce(&Self)->Result<T>>(&self,action:F)->Result<T>{
        let path=self.root.join(".atlas/teams/.registry.lock");
        if path.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()){
            bail!("Unsafe registry lock")
        }
        let lock=OpenOptions::new().create(true).write(true).open(path)?;
        lock.lock_exclusive()?;
        let result=action(self);
        let _=FileExt::unlock(&lock);
        result
    }
    fn path(&self,name:&str)->Result<PathBuf>{
        if name.trim().is_empty()||name.len()>120{bail!("Invalid team name")}
        let safe=name.chars().map(|c|if c.is_ascii_alphanumeric()||c=='_'||c=='-'{
            c
        }else{'-'}).collect::<String>();
        let dir=self.root.join(".atlas/teams").join(safe);
        fs::create_dir_all(&dir)?;
        if !dir.canonicalize()?.starts_with(self.root.join(".atlas/teams").canonicalize()?){
            bail!("Team directory escapes workspace")
        }
        let path=dir.join("config.json");
        if path.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()){
            bail!("Unsafe team config link")
        }
        Ok(path)
    }
    pub fn config_path(&self,name:&str)->Result<PathBuf>{self.path(name)}
    pub fn read(&self,name:&str)->Result<Option<Team>>{
        let path=self.path(name)?;
        match fs::read(path){
            Ok(raw)=>Ok(Some(serde_json::from_slice(&raw)?)),
            Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),
            Err(e)=>Err(e.into())
        }
    }
    fn save(&self,team:&Team)->Result<()>{
        let path=self.path(&team.name)?;
        let data=serde_json::to_vec_pretty(team)?;
        let temp=path.with_extension(format!("{}.tmp",uuid::Uuid::new_v4()));
        let mut file=OpenOptions::new().write(true).create_new(true).open(&temp)?;
        let result=(||->Result<()>{
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temp,&path)?;
            Ok(())
        })();
        if result.is_err(){let _=fs::remove_file(&temp);}
        result
    }
    pub fn create(&self,name:&str,description:Option<&str>,session:&str)->Result<Team>{
        self.locked(|this|{
        let mut name=name.trim().to_owned();
        if this.read(&name)?.is_some(){
            name=format!("{}-{}",&name,uuid::Uuid::new_v4().simple());
        }
        let lead=format!("team-lead@{name}");
        let team=Team{
            name,description:description.map(str::to_owned),
            lead_session:session.into(),lead_id:lead.clone(),
            members:vec![Member{name:"team-lead".into(),id:lead,active:true}],
        };
        if this.read(&team.name)?.is_some(){bail!("Duplicate team")}
        this.save(&team)?;
        Ok(team)
        })
    }
    pub fn member(&self,team:&str,member:Member)->Result<()>{
        self.locked(|this|{
        if member.name.is_empty() || member.name.len()>80 ||
            !member.name.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'||c=='-'){
            bail!("Invalid member name")
        }
        let mut cfg=this.read(team)?.context("Team missing")?;
        if cfg.members.iter().any(|m|m.name==member.name||m.id==member.id){
            bail!("Duplicate member")
        }
        cfg.members.push(member);
        this.save(&cfg)
        })
    }
    pub fn active(&self,team:&str,name:&str,active:bool)->Result<()>{
        self.locked(|this|{
        let mut cfg=this.read(team)?.context("Team missing")?;
        let member=cfg.members.iter_mut().find(|m|m.name==name)
            .context("Member missing")?;
        member.active=active;
        this.save(&cfg)
        })
    }
    pub fn delete(&self,team:Option<&str>)->Result<Value>{
        self.locked(|this|{
        let Some(name)=team else {
            return Ok(json!({"success":true,"message":"No team name found, nothing to clean up"}))
        };
        let cfg=this.read(name)?.context("Team missing")?;
        let active=cfg.members.iter().filter(|m|m.name!="team-lead"&&m.active)
            .map(|m|m.name.as_str()).collect::<Vec<_>>();
        if !active.is_empty(){
            return Ok(json!({"success":false,"team_name":name,
                "message":format!("Cannot cleanup team with {} active member(s): {}. Use requestShutdown to gracefully terminate teammates first.",active.len(),active.join(", "))}))
        }
        let cfg_path=this.path(name)?;
        fs::remove_dir_all(cfg_path.parent().context("Team directory missing")?)?;
        Ok(json!({"success":true,"team_name":name,
            "message":format!("Cleaned up directories and worktrees for team \"{name}\"")}))
        })
    }
    pub fn send(&self,team:&str,recipient:&str,sender:&str,text:&str,summary:&str)->Result<Value>{
        if summary.trim().is_empty(){bail!("summary is required when message is a string")}
        self.locked(|this|{
        let cfg=this.read(team)?.context("Team missing")?;
        if !cfg.members.iter().any(|m|m.name==sender){bail!("Sender is not a team member")}
        let names=if recipient=="*"{
            cfg.members.iter().filter(|m|m.name!=sender)
                .map(|m|m.name.clone()).collect::<Vec<_>>()
        }else{
            vec![cfg.members.iter().find(|m|m.name==recipient||m.id==recipient)
                .context("Unknown recipient")?.name.clone()]
        };
        let inbox=Mailbox::new(&this.root,team)?;
        for name in &names {
            inbox.write(name,Message{
                from:sender.into(),text:text.into(),read:false,
                timestamp:chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
                color:None,summary:Some(summary.into())
            })?;
        }
        Ok(json!({"success":true,"recipients":names}))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_members_broadcast_and_cleanup_guards(){
        let root=tempfile::tempdir().unwrap();
        let teams=Teams::new(root.path()).unwrap();
        let team=teams.create("project",Some("Review"),"session").unwrap();
        teams.member(&team.name,Member{
            name:"worker".into(),id:"worker@project".into(),active:true
        }).unwrap();
        assert_eq!(teams.send("project","*","team-lead","Review now","Review").unwrap()["recipients"],
            json!(["worker"]));
        let mailbox=Mailbox::new(root.path(),"project").unwrap();
        assert_eq!(mailbox.unread("worker").unwrap().len(),1);
        assert_eq!(teams.delete(Some("project")).unwrap()["success"],false);
        teams.active("project","worker",false).unwrap();
        assert_eq!(teams.delete(Some("project")).unwrap()["success"],true);
    }
}

pub fn definitions(leader:bool,team_active:bool)->Vec<Value>{
    let mut result=Vec::new();
    if leader {
        result.push(json!({"type":"function","function":{
            "name":"TeamCreate",
            "description":"Create a persistent local multi-agent team and task list.",
            "parameters":{"type":"object","properties":{
                "team_name":{"type":"string"},"description":{"type":"string"}
            },"required":["team_name"]}
        }}));
        result.push(json!({"type":"function","function":{
            "name":"TeamDelete",
            "description":"Clean up a team after all teammates have stopped.",
            "parameters":{"type":"object","properties":{}}
        }}));
    }
    if team_active {
        result.push(json!({"type":"function","function":{
            "name":"SendMessage",
            "description":"Send a plain-text message to a teammate or broadcast to all other members. A short summary is required.",
            "parameters":{"type":"object","properties":{
                "to":{"type":"string"},
                "summary":{"type":"string"},
                "message":{"type":"string"}
            },"required":["to","summary","message"]}
        }}));
        result.push(json!({"type":"function","function":{
            "name":"TeamInbox",
            "description":"Read your team's local mailbox, optionally acknowledging messages.",
            "parameters":{"type":"object","properties":{
                "recipient":{"type":"string"},
                "mark_read":{"type":"boolean"}
            }}
        }}));
    }
    result
}

pub fn is_reserved_protocol(text:&str)->bool{
    let Ok(value)=serde_json::from_str::<Value>(text) else{return false};
    matches!(value.get("type").and_then(Value::as_str),
        Some("permission_request"|"permission_response"|
             "sandbox_permission_request"|"sandbox_permission_response"|
             "shutdown_request"|"shutdown_approved"|"shutdown_rejected"|
             "plan_approval_request"|"plan_approval_response"|
             "team_permission_update"|"mode_set_request"|
             "idle_notification"|"task_assignment"))
}
fn escape_attr(text:&str)->String{
    text.replace('&',"&amp;").replace('"',"&quot;")
        .replace('<',"&lt;").replace('>',"&gt;")
}
pub fn format_incoming(messages:&[Message])->String{
    messages.iter().map(|m|{
        let color=m.color.as_deref().map(|x|format!(" color=\"{}\"",escape_attr(x))).unwrap_or_default();
        let summary=m.summary.as_deref().map(|x|format!(" summary=\"{}\"",escape_attr(x))).unwrap_or_default();
        format!("<teammate-message teammate_id=\"{}\"{color}{summary}>\n{}\n</teammate-message>",
            escape_attr(&m.from),m.text)
    }).collect::<Vec<_>>().join("\n\n")
}
#[cfg(test)]
mod message_tests{
    use super::*;
    #[test]
    fn control_protocol_messages_are_not_plain_chat(){
        assert!(is_reserved_protocol(r#"{"type":"permission_request"}"#));
        assert!(is_reserved_protocol(r#"{"type":"shutdown_request"}"#));
        assert!(!is_reserved_protocol(r#"{"type":"other"}"#));
        assert!(!is_reserved_protocol("ordinary teammate message"));
    }
    #[test]
    fn incoming_messages_escape_untrusted_attributes(){
        let input=Message{from:"a\"<".into(),text:"review".into(),
            timestamp:"now".into(),read:false,color:None,
            summary:Some("x&y".into())};
        let formatted=format_incoming(&[input]);
        assert!(formatted.contains("teammate_id=\"a&quot;&lt;\""));
        assert!(formatted.contains("summary=\"x&amp;y\""));
    }
}
