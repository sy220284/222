use anyhow::{bail,Context,Result};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf}};
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
        fs::write(path,serde_json::to_vec_pretty(team)?)?;
        Ok(())
    }
    pub fn create(&self,name:&str,description:Option<&str>,session:&str)->Result<Team>{
        let mut name=name.trim().to_owned();
        if self.read(&name)?.is_some(){
            name=format!("{}-{}",&name,uuid::Uuid::new_v4().simple());
        }
        let lead=format!("team-lead@{name}");
        let team=Team{
            name,description:description.map(str::to_owned),
            lead_session:session.into(),lead_id:lead.clone(),
            members:vec![Member{name:"team-lead".into(),id:lead,active:true}],
        };
        if self.read(&team.name)?.is_some(){bail!("Duplicate team")}
        self.save(&team)?;
        Ok(team)
    }
    pub fn member(&self,team:&str,member:Member)->Result<()>{
        let mut cfg=self.read(team)?.context("Team missing")?;
        if cfg.members.iter().any(|m|m.name==member.name||m.id==member.id){
            bail!("Duplicate member")
        }
        cfg.members.push(member);
        self.save(&cfg)
    }
    pub fn active(&self,team:&str,name:&str,active:bool)->Result<()>{
        let mut cfg=self.read(team)?.context("Team missing")?;
        let member=cfg.members.iter_mut().find(|m|m.name==name)
            .context("Member missing")?;
        member.active=active;
        self.save(&cfg)
    }
    pub fn delete(&self,team:Option<&str>)->Result<Value>{
        let Some(name)=team else {
            return Ok(json!({"success":true,"message":"No team name found, nothing to clean up"}))
        };
        let cfg=self.read(name)?.context("Team missing")?;
        let active=cfg.members.iter().filter(|m|m.name!="team-lead"&&m.active)
            .map(|m|m.name.as_str()).collect::<Vec<_>>();
        if !active.is_empty(){
            return Ok(json!({"success":false,"team_name":name,
                "message":format!("Cannot cleanup team with {} active member(s): {}. Use requestShutdown to gracefully terminate teammates first.",active.len(),active.join(", "))}))
        }
        let cfg_path=self.path(name)?;
        fs::remove_dir_all(cfg_path.parent().context("Team directory missing")?)?;
        Ok(json!({"success":true,"team_name":name,
            "message":format!("Cleaned up directories and worktrees for team \"{name}\"")}))
    }
    pub fn send(&self,team:&str,recipient:&str,sender:&str,text:&str,summary:&str)->Result<Value>{
        let cfg=self.read(team)?.context("Team missing")?;
        let names=if recipient=="*"{
            cfg.members.iter().filter(|m|m.name!=sender)
                .map(|m|m.name.clone()).collect::<Vec<_>>()
        }else{
            vec![cfg.members.iter().find(|m|m.name==recipient||m.id==recipient)
                .context("Unknown recipient")?.name.clone()]
        };
        let inbox=Mailbox::new(&self.root,team)?;
        for name in &names {
            inbox.write(name,Message{
                from:sender.into(),text:text.into(),read:false,
                timestamp:chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
                color:None,summary:Some(summary.into())
            })?;
        }
        Ok(json!({"success":true,"recipients":names}))
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
