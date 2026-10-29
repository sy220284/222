//! Durable queue-operation audit records, kept beside workspace sessions.
//! This is not a replay queue: it preserves the original operation event trail.
use anyhow::{bail,Context,Result};
use fs2::FileExt;
use serde_json::json;
use std::{fs::{self,OpenOptions},io::Write,path::{Path,PathBuf}};
use uuid::Uuid;

pub struct Journal{
    path:PathBuf,
    session_id:String,
}
impl Journal{
    pub fn new(root:&Path,session_id:&str)->Result<Self>{
        let id=Uuid::parse_str(session_id).context("Invalid queue session UUID")?;
        let root=root.canonicalize().context("Workspace unavailable")?;
        let base=root.join(".atlas/queue");
        fs::create_dir_all(&base)?;
        let real=base.canonicalize()?;
        if !real.starts_with(&root){bail!("Queue journal escapes workspace")}
        let path=real.join(format!("{id}.jsonl"));
        if path.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()){
            bail!("Unsafe queue journal symlink")
        }
        Ok(Self{path,session_id:id.to_string()})
    }
    pub fn record(&self,operation:&str,content:Option<&str>)->Result<()>{
        if self.path.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()){
            bail!("Unsafe queue journal symlink")
        }
        let mut options=OpenOptions::new();
        options.create(true).append(true).read(true);
        #[cfg(unix)]{
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file=options.open(&self.path)?;
        file.lock_exclusive()?;
        let mut entry=json!({
            "type":"queue-operation","operation":operation,
            "timestamp":chrono::Utc::now().to_rfc3339(),
            "sessionId":self.session_id
        });
        if let Some(content)=content{entry["content"]=json!(content)}
        let outcome=(||->Result<()>{
            serde_json::to_writer(&mut file,&entry)?;
            file.write_all(b"\n")?;
            file.sync_data()?;
            Ok(())
        })();
        let unlock=FileExt::unlock(&file);
        outcome?;
        unlock?;
        Ok(())
    }
    #[cfg(test)]
    fn path(&self)->&Path{&self.path}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn writes_ordered_jsonl_events_and_preserves_text(){
        let dir=tempfile::tempdir().unwrap();
        let id=Uuid::new_v4().to_string();
        let journal=Journal::new(dir.path(),&id).unwrap();
        journal.record("enqueue",Some("a\nb")).unwrap();
        journal.record("dequeue",None).unwrap();
        let records=fs::read_to_string(journal.path()).unwrap();
        let events=records.lines().map(|s|serde_json::from_str::<serde_json::Value>(s).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(events.len(),2);
        assert_eq!(events[0]["operation"],"enqueue");
        assert_eq!(events[0]["content"],"a\nb");
        assert_eq!(events[1]["operation"],"dequeue");
        assert_eq!(events[1]["sessionId"],id);
    }
    #[cfg(unix)]
    #[test]
    fn rejects_journal_symlink_escape(){
        use std::os::unix::fs::symlink;
        let root=tempfile::tempdir().unwrap();
        let outside=tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas")).unwrap();
        symlink(outside.path(),root.path().join(".atlas/queue")).unwrap();
        assert!(Journal::new(root.path(),&Uuid::new_v4().to_string()).is_err());
    }
}
