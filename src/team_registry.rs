use crate::team_mailbox::{Mailbox, Message};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub id: String,
    pub active: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub description: Option<String>,
    pub lead_session: String,
    pub lead_id: String,
    pub members: Vec<Member>,
}
pub struct Teams {
    root: PathBuf,
}
impl Teams {
    pub fn new(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        let teams = root.join(".atlas/teams");
        fs::create_dir_all(&teams)?;
        if !teams.canonicalize()?.starts_with(&root) {
            bail!("Teams escape workspace")
        }
        Ok(Self { root })
    }
    fn locked<T, F: FnOnce(&Self) -> Result<T>>(&self, action: F) -> Result<T> {
        let path = self.root.join(".atlas/teams/.registry.lock");
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("Unsafe registry lock")
        }
        let lock = OpenOptions::new().create(true).write(true).truncate(false).open(path)?;
        lock.lock_exclusive()?;
        let result = action(self);
        let _ = FileExt::unlock(&lock);
        result
    }
    fn path(&self, name: &str) -> Result<PathBuf> {
        if name.trim().is_empty() || name.len() > 120 {
            bail!("Invalid team name")
        }
        let safe = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let dir = self.root.join(".atlas/teams").join(safe);
        fs::create_dir_all(&dir)?;
        if !dir
            .canonicalize()?
            .starts_with(self.root.join(".atlas/teams").canonicalize()?)
        {
            bail!("Team directory escapes workspace")
        }
        let path = dir.join("config.json");
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("Unsafe team config link")
        }
        Ok(path)
    }
    pub fn config_path(&self, name: &str) -> Result<PathBuf> {
        self.path(name)
    }
    pub fn read(&self, name: &str) -> Result<Option<Team>> {
        let path = self.path(name)?;
        match fs::read(path) {
            Ok(raw) => Ok(Some(serde_json::from_slice(&raw)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn save(&self, team: &Team) -> Result<()> {
        let path = self.path(&team.name)?;
        let data = serde_json::to_vec_pretty(team)?;
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let result = (|| -> Result<()> {
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    pub fn create(&self, name: &str, description: Option<&str>, session: &str) -> Result<Team> {
        self.locked(|this| {
            let mut name = name.trim().to_owned();
            if this.read(&name)?.is_some() {
                name = format!("{}-{}", &name, uuid::Uuid::new_v4().simple());
            }
            let lead = format!("team-lead@{name}");
            let team = Team {
                name,
                description: description.map(str::to_owned),
                lead_session: session.into(),
                lead_id: lead.clone(),
                members: vec![Member {
                    name: "team-lead".into(),
                    id: lead,
                    active: true,
                }],
            };
            if this.read(&team.name)?.is_some() {
                bail!("Duplicate team")
            }
            this.save(&team)?;
            Ok(team)
        })
    }
    pub fn member(&self, team: &str, member: Member) -> Result<()> {
        self.locked(|this| {
            if member.name.is_empty()
                || member.name.len() > 80
                || !member
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                bail!("Invalid member name")
            }
            let mut cfg = this.read(team)?.context("Team missing")?;
            if cfg
                .members
                .iter()
                .any(|m| m.id == member.id && m.name != member.name)
            {
                bail!("Duplicate member identity")
            }
            if let Some(existing) = cfg.members.iter_mut().find(|m| m.name == member.name) {
                if existing.active {
                    bail!("Duplicate active member")
                }
                existing.id = member.id;
                existing.active = true;
            } else {
                cfg.members.push(member);
            }
            this.save(&cfg)
        })
    }
    pub fn active(&self, team: &str, name: &str, active: bool) -> Result<()> {
        self.locked(|this| {
            let mut cfg = this.read(team)?.context("Team missing")?;
            let member = cfg
                .members
                .iter_mut()
                .find(|m| m.name == name)
                .context("Member missing")?;
            member.active = active;
            this.save(&cfg)
        })
    }
    pub fn delete(&self, team: Option<&str>) -> Result<Value> {
        self.locked(|this|{
        let Some(name)=team else {
            return Ok(json!({"success":true,"message":"No team name found, nothing to clean up"}))
        };
        let cfg=match this.read(name)?{
            Some(cfg)=>cfg,
            None=>{
                let path=this.path(name)?;
                if let Some(directory)=path.parent(){
                    if directory.exists(){fs::remove_dir_all(directory)?;}
                }
                return Ok(json!({"success":true,"team_name":name,
                    "message":"Team configuration already removed"}));
            }
        };
        let active=cfg.members.iter().filter(|m|m.name!="team-lead"&&m.active)
            .map(|m|m.name.as_str()).collect::<Vec<_>>();
        if !active.is_empty(){
            return Ok(json!({"success":false,"team_name":name,
                "message":format!("Cannot cleanup team with {} active member(s): {}. Use requestShutdown to gracefully terminate teammates first.",active.len(),active.join(", "))}))
        }
        let cfg_path=this.path(name)?;
        fs::remove_dir_all(cfg_path.parent().context("Team directory missing")?)?;
        Ok(json!({"success":true,"team_name":name,
            "message":format!("Cleaned up team configuration and inbox directory for team \"{name}\"")}))
        })
    }
    /// Structured peer protocol. The on-disk payload field names match
    /// the source protocol; it never routes to a vendor service.
    pub fn send_structured(
        &self,
        team: &str,
        recipient: &str,
        sender: &str,
        input: &Value,
    ) -> Result<Value> {
        let kind = input
            .get("type")
            .and_then(Value::as_str)
            .context("Structured message type required")?;
        if recipient == "*" {
            bail!("structured messages cannot be broadcast")
        }
        self.locked(|this| {
            let cfg = this.read(team)?.context("Team missing")?;
            if !cfg.members.iter().any(|m| m.name == sender && m.active) {
                bail!("Sender is not an active team member")
            }
            let target_member = cfg
                .members
                .iter()
                .find(|m| m.name == recipient || m.id == recipient)
                .context("Unknown recipient")?;
            if !target_member.active {
                bail!("Recipient is inactive")
            }
            let target = target_member.name.clone();
            let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let (payload, request_id, shutdown) = match kind {
                "shutdown_request" => {
                    if sender != "team-lead" || target == "team-lead" {
                        bail!("Only the team lead can request a teammate shutdown")
                    }
                    let reason = input.get("reason").and_then(Value::as_str);
                    let request_id = format!(
                        "shutdown-{}-{}@{}",
                        chrono::Utc::now().timestamp_millis(),
                        uuid::Uuid::new_v4().simple(),
                        target
                    );
                    (
                        json!({"type":"shutdown_request","requestId":request_id,
                        "from":sender,"reason":reason,"timestamp":timestamp}),
                        request_id,
                        false,
                    )
                }
                "shutdown_response" => {
                    if target != "team-lead" || sender == "team-lead" {
                        bail!("shutdown_response must be sent by a teammate to team-lead")
                    }
                    let request_id = input
                        .get("request_id")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .context("request_id is required")?
                        .to_owned();
                    // A teammate may only answer a real, persisted request
                    // addressed to that teammate. The team registry lock
                    // serializes duplicate responses across agent processes.
                    let inbox = Mailbox::new(&this.root, team)?;
                    let issued = inbox.read(sender)?.iter().any(|item| {
                        if item.from != "team-lead" {
                            return false;
                        }
                        serde_json::from_str::<Value>(&item.text)
                            .ok()
                            .is_some_and(|message| {
                                message.get("type").and_then(Value::as_str)
                                    == Some("shutdown_request")
                                    && message.get("requestId").and_then(Value::as_str)
                                        == Some(&request_id)
                                    && message.get("from").and_then(Value::as_str)
                                        == Some("team-lead")
                            })
                    });
                    if !issued {
                        bail!("No matching shutdown request for teammate")
                    }
                    let answered = inbox.read("team-lead")?.iter().any(|item| {
                        if item.from != sender {
                            return false;
                        }
                        serde_json::from_str::<Value>(&item.text)
                            .ok()
                            .is_some_and(|message| {
                                matches!(
                                    message.get("type").and_then(Value::as_str),
                                    Some("shutdown_approved" | "shutdown_rejected")
                                ) && message.get("requestId").and_then(Value::as_str)
                                    == Some(&request_id)
                            })
                    });
                    if answered {
                        bail!("Shutdown request already answered")
                    }
                    let approve =
                        semantic_bool(input.get("approve").context("approve is required")?)?;
                    let payload = if approve {
                        json!({"type":"shutdown_approved","requestId":request_id,
                            "from":sender,"timestamp":timestamp})
                    } else {
                        let reason = input
                            .get("reason")
                            .and_then(Value::as_str)
                            .filter(|s| !s.trim().is_empty())
                            .context("reason is required when rejecting shutdown")?;
                        json!({"type":"shutdown_rejected","requestId":request_id,
                            "from":sender,"reason":reason,"timestamp":timestamp})
                    };
                    (payload, request_id, approve)
                }
                "mode_set_request" => {
                    if sender != "team-lead" || target == "team-lead" {
                        bail!("Only the team lead can set a teammate permission mode")
                    }
                    let mode = input
                        .get("mode")
                        .and_then(Value::as_str)
                        .context("mode is required")?;
                    if crate::permission_mode::Mode::parse(mode).is_none() {
                        bail!("Unknown teammate permission mode")
                    }
                    let request_id = format!("mode-{}", uuid::Uuid::new_v4().simple());
                    (
                        json!({"type":"mode_set_request","mode":mode,
                        "from":"team-lead"}),
                        request_id,
                        false,
                    )
                }
                "plan_approval_request" => {
                    if sender == "team-lead" || target != "team-lead" {
                        bail!("Only a teammate can request plan approval from team-lead")
                    }
                    let plan_file = input
                        .get("plan_file_path")
                        .or_else(|| input.get("planFilePath"))
                        .and_then(Value::as_str)
                        .context("plan_file_path is required")?;
                    let plan_content = input
                        .get("plan_content")
                        .or_else(|| input.get("planContent"))
                        .and_then(Value::as_str)
                        .context("plan_content is required")?;
                    let request_id = input
                        .get("request_id")
                        .or_else(|| input.get("requestId"))
                        .and_then(Value::as_str)
                        .filter(|id| !id.trim().is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("plan-{}", uuid::Uuid::new_v4().simple()));
                    (
                        json!({"type":"plan_approval_request","requestId":request_id,
                        "from":sender,"timestamp":timestamp,"planFilePath":plan_file,
                        "planContent":plan_content}),
                        request_id,
                        false,
                    )
                }
                "plan_approval_response" => {
                    if sender != "team-lead" {
                        bail!("Only the team lead can approve plans")
                    }
                    let request_id = input
                        .get("request_id")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .context("request_id is required")?
                        .to_owned();
                    let approve =
                        semantic_bool(input.get("approve").context("approve is required")?)?;
                    // A leader can only approve a persisted request addressed to
                    // them by this exact worker. The registry lock serializes
                    // duplicate replies across concurrent team processes.
                    let inbox = Mailbox::new(&this.root, team)?;
                    let issued = inbox.read("team-lead")?.iter().any(|item| {
                        if item.from != target {
                            return false;
                        }
                        serde_json::from_str::<Value>(&item.text)
                            .ok()
                            .is_some_and(|message| {
                                message.get("type").and_then(Value::as_str)
                                    == Some("plan_approval_request")
                                    && message.get("requestId").and_then(Value::as_str)
                                        == Some(&request_id)
                                    && message.get("from").and_then(Value::as_str) == Some(&target)
                            })
                    });
                    if !issued {
                        bail!("No matching plan approval request from teammate")
                    }
                    let answered = inbox.read(&target)?.iter().any(|item| {
                        if item.from != "team-lead" {
                            return false;
                        }
                        serde_json::from_str::<Value>(&item.text)
                            .ok()
                            .is_some_and(|message| {
                                message.get("type").and_then(Value::as_str)
                                    == Some("plan_approval_response")
                                    && message.get("requestId").and_then(Value::as_str)
                                        == Some(&request_id)
                            })
                    });
                    if answered {
                        bail!("Plan approval request already answered")
                    }
                    let feedback = input.get("feedback").and_then(Value::as_str);
                    let mode = input.get("permission_mode").and_then(Value::as_str);
                    if let Some(mode) = mode {
                        if crate::permission_mode::Mode::parse(mode).is_none() {
                            bail!("Unknown plan approval permission mode")
                        }
                    }
                    (
                        json!({"type":"plan_approval_response","requestId":request_id,
                        "approved":approve,"feedback":feedback,"timestamp":timestamp,
                        "permissionMode":mode}),
                        request_id,
                        false,
                    )
                }
                other => bail!("Unsupported structured message type: {other}"),
            };
            let inbox = Mailbox::new(&this.root, team)?;
            inbox.write(
                &target,
                Message {
                    from: sender.into(),
                    text: payload.to_string(),
                    timestamp,
                    read: false,
                    color: None,
                    summary: None,
                },
            )?;
            Ok(json!({"success":true,"request_id":request_id,
                "target":target,"shutdown_requested":shutdown}))
        })
    }
    /// Mirror the original leader inbox poller: approve teammate plans and
    /// persist a correlated response before the request is acknowledged.
    /// Re-reading a request after a crash is safe: an existing response is
    /// recognized and not sent again.
    pub fn approve_pending_plans(&self, team: &str, leader_mode: &str) -> Result<usize> {
        let mode = if leader_mode == "plan" {
            "default"
        } else {
            leader_mode
        };
        if crate::permission_mode::Mode::parse(mode).is_none() {
            bail!("Unknown leader permission mode")
        }
        let inbox = Mailbox::new(&self.root, team)?;
        let mut approved = 0;
        for item in inbox.unread("team-lead")? {
            let Ok(payload) = serde_json::from_str::<Value>(&item.text) else {
                continue;
            };
            if payload.get("type").and_then(Value::as_str) != Some("plan_approval_request")
                || item.from == "team-lead"
                || payload.get("from").and_then(Value::as_str) != Some(&item.from)
            {
                continue;
            }
            let Some(request_id) = payload
                .get("requestId")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let already_answered = || -> Result<bool> {
                Ok(inbox.read(&item.from)?.iter().any(|answer| {
                    answer.from == "team-lead"
                        && serde_json::from_str::<Value>(&answer.text)
                            .ok()
                            .is_some_and(|v| {
                                v.get("type").and_then(Value::as_str)
                                    == Some("plan_approval_response")
                                    && v.get("requestId").and_then(Value::as_str)
                                        == Some(request_id)
                            })
                }))
            };
            if already_answered()? {
                continue;
            }
            let reply = json!({"type":"plan_approval_response",
                "request_id":request_id,"approve":true,"permission_mode":mode});
            match self.send_structured(team, &item.from, "team-lead", &reply) {
                Ok(_) => approved += 1,
                Err(error) => {
                    // A concurrent leader poll may have answered under the
                    // same registry lock. Do not fail a valid model turn.
                    if !already_answered()? {
                        return Err(error);
                    }
                }
            }
        }
        Ok(approved)
    }
    pub fn send(
        &self,
        team: &str,
        recipient: &str,
        sender: &str,
        text: &str,
        summary: &str,
    ) -> Result<Value> {
        if summary.trim().is_empty() {
            bail!("summary is required when message is a string")
        }
        self.locked(|this| {
            let cfg = this.read(team)?.context("Team missing")?;
            if !cfg.members.iter().any(|m| m.name == sender && m.active) {
                bail!("Sender is not an active team member")
            }
            let names = if recipient == "*" {
                cfg.members
                    .iter()
                    .filter(|m| m.name != sender && m.active)
                    .map(|m| m.name.clone())
                    .collect::<Vec<_>>()
            } else {
                let member = cfg
                    .members
                    .iter()
                    .find(|m| m.name == recipient || m.id == recipient)
                    .context("Unknown recipient")?;
                if !member.active {
                    bail!("Recipient is inactive")
                }
                vec![member.name.clone()]
            };
            let inbox = Mailbox::new(&this.root, team)?;
            for name in &names {
                inbox.write(
                    name,
                    Message {
                        from: sender.into(),
                        text: text.into(),
                        read: false,
                        timestamp: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        color: None,
                        summary: Some(summary.into()),
                    },
                )?;
            }
            Ok(json!({"success":true,"recipients":names}))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stopped_member_can_rejoin_without_duplicate_roster_entry() {
        let root = tempfile::tempdir().unwrap();
        let registry = Teams::new(root.path()).unwrap();
        registry.create("team", None, "lead").unwrap();
        let member = Member {
            name: "worker".into(),
            id: "worker@team".into(),
            active: true,
        };
        registry.member("team", member.clone()).unwrap();
        assert!(registry.member("team", member.clone()).is_err());
        registry.active("team", "worker", false).unwrap();
        registry.member("team", member).unwrap();
        let cfg = registry.read("team").unwrap().unwrap();
        assert_eq!(cfg.members.len(), 2);
        assert!(cfg.members[1].active);
    }
    #[test]
    fn concurrent_member_updates_are_serialized_and_durable() {
        let root = std::sync::Arc::new(tempfile::tempdir().unwrap());
        Teams::new(root.path())
            .unwrap()
            .create("parallel", None, "lead")
            .unwrap();
        let workers = (0..16)
            .map(|i| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let registry = Teams::new(root.path()).unwrap();
                    registry
                        .member(
                            "parallel",
                            Member {
                                name: format!("worker-{i}"),
                                id: format!("worker-{i}@parallel"),
                                active: true,
                            },
                        )
                        .unwrap();
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let team = Teams::new(root.path())
            .unwrap()
            .read("parallel")
            .unwrap()
            .unwrap();
        assert_eq!(team.members.len(), 17);
        assert!(team.members.iter().all(|m| m.active));
    }
    #[test]
    fn team_members_broadcast_and_cleanup_guards() {
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        let team = teams.create("project", Some("Review"), "session").unwrap();
        teams
            .member(
                &team.name,
                Member {
                    name: "worker".into(),
                    id: "worker@project".into(),
                    active: true,
                },
            )
            .unwrap();
        assert_eq!(
            teams
                .send("project", "*", "team-lead", "Review now", "Review")
                .unwrap()["recipients"],
            json!(["worker"])
        );
        let mailbox = Mailbox::new(root.path(), "project").unwrap();
        assert_eq!(mailbox.unread("worker").unwrap().len(), 1);
        assert_eq!(teams.delete(Some("project")).unwrap()["success"], false);
        teams.active("project", "worker", false).unwrap();
        assert_eq!(teams.delete(Some("project")).unwrap()["success"], true);
    }
}

pub fn definitions(leader: bool, team_active: bool) -> Vec<Value> {
    let mut result = Vec::new();
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
            "description":"Send a teammate a plain message, shutdown request/response or correlated plan approval request/response. Plain messages require a short summary; broadcast supports only plain messages.",
            "parameters":{"type":"object","properties":{
                "to":{"type":"string"},
                "summary":{"type":"string"},
                "message":{"oneOf":[
                    {"type":"string"},
                    {"type":"object","properties":{
                        "type":{"const":"shutdown_request"},"reason":{"type":"string"}
                    },"required":["type"]},
                    {"type":"object","properties":{
                        "type":{"const":"shutdown_response"},
                        "request_id":{"type":"string"},"approve":{"type":"boolean"},
                        "reason":{"type":"string"}
                    },"required":["type","request_id","approve"]},
                    {"type":"object","properties":{
                        "type":{"const":"mode_set_request"},"mode":{"type":"string"}
                    },"required":["type","mode"]},
                    {"type":"object","properties":{
                        "type":{"const":"plan_approval_request"},
                        "request_id":{"type":"string"},
                        "plan_file_path":{"type":"string"},"plan_content":{"type":"string"}
                    },"required":["type","plan_file_path","plan_content"]},
                    {"type":"object","properties":{
                        "type":{"const":"plan_approval_response"},
                        "request_id":{"type":"string"},"approve":{"type":"boolean"},
                        "feedback":{"type":"string"},"permission_mode":{"type":"string"}
                    },"required":["type","request_id","approve"]}
                ]}
            },"required":["to","message"]}
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

pub fn is_reserved_protocol(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    matches!(
        value.get("type").and_then(Value::as_str),
        Some(
            "permission_request"
                | "permission_response"
                | "sandbox_permission_request"
                | "sandbox_permission_response"
                | "shutdown_request"
                | "shutdown_approved"
                | "plan_approval_request"
                | "plan_approval_response"
                | "team_permission_update"
                | "mode_set_request"
        )
    )
}
/// Protocol requests requiring a human/model decision can be surfaced as
/// peer context; permission and mode updates stay unread until their
/// dedicated authorization handlers exist.
pub fn is_model_deliverable(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return true;
    };
    !matches!(
        value.get("type").and_then(Value::as_str),
        Some(
            "permission_request"
                | "permission_response"
                | "sandbox_permission_request"
                | "sandbox_permission_response"
                | "team_permission_update"
                | "mode_set_request"
                | "plan_approval_response"
        )
    )
}
fn escape_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
pub fn format_incoming(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|m| {
            let color = m
                .color
                .as_deref()
                .map(|x| format!(" color=\"{}\"", escape_attr(x)))
                .unwrap_or_default();
            let summary = m
                .summary
                .as_deref()
                .map(|x| format!(" summary=\"{}\"", escape_attr(x)))
                .unwrap_or_default();
            format!(
                "<teammate-message teammate_id=\"{}\"{color}{summary}>\n{}\n</teammate-message>",
                escape_attr(&m.from),
                m.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
#[cfg(test)]
mod message_tests {
    use super::*;
    #[test]
    fn control_protocol_messages_are_not_plain_chat() {
        assert!(is_reserved_protocol(r#"{"type":"permission_request"}"#));
        assert!(is_reserved_protocol(r#"{"type":"shutdown_request"}"#));
        assert!(!is_reserved_protocol(r#"{"type":"other"}"#));
        assert!(!is_reserved_protocol("ordinary teammate message"));
        assert!(!is_reserved_protocol(r#"{"type":"task_assignment"}"#));
        assert!(is_model_deliverable(r#"{"type":"shutdown_request"}"#));
        assert!(is_model_deliverable(r#"{"type":"task_assignment"}"#));
        assert!(!is_model_deliverable(r#"{"type":"permission_response"}"#));
    }
    #[test]
    fn incoming_messages_escape_untrusted_attributes() {
        let input = Message {
            from: "a\"<".into(),
            text: "review".into(),
            timestamp: "now".into(),
            read: false,
            color: None,
            summary: Some("x&y".into()),
        };
        let formatted = format_incoming(&[input]);
        assert!(formatted.contains("teammate_id=\"a&quot;&lt;\""));
        assert!(formatted.contains("summary=\"x&amp;y\""));
    }
}

fn semantic_bool(value: &Value) -> Result<bool> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::String(value) if value == "true" => Ok(true),
        Value::String(value) if value == "false" => Ok(false),
        _ => bail!("approve must be a boolean or literal true/false"),
    }
}
#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn structured_shutdown_and_plan_protocol_roundtrip() {
        let root = tempfile::tempdir().unwrap();
        let registry = Teams::new(root.path()).unwrap();
        registry.create("team", None, "lead").unwrap();
        registry
            .member(
                "team",
                Member {
                    name: "worker".into(),
                    id: "worker@team".into(),
                    active: true,
                },
            )
            .unwrap();
        let request = registry
            .send_structured(
                "team",
                "worker",
                "team-lead",
                &json!({"type":"shutdown_request","reason":"done"}),
            )
            .unwrap();
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        let message = inbox.unread("worker").unwrap();
        assert_eq!(message.len(), 1);
        let payload: Value = serde_json::from_str(&message[0].text).unwrap();
        assert_eq!(payload["type"], "shutdown_request");
        assert_eq!(payload["requestId"], request["request_id"]);
        let approval = registry
            .send_structured(
                "team",
                "team-lead",
                "worker",
                &json!({"type":"shutdown_response","request_id":request["request_id"],
                "approve":"true"}),
            )
            .unwrap();
        assert_eq!(approval["shutdown_requested"], true);
        let response: Value =
            serde_json::from_str(&inbox.unread("team-lead").unwrap()[0].text).unwrap();
        assert_eq!(response["type"], "shutdown_approved");
        assert!(registry
            .send_structured("team", "*", "worker", &json!({"type":"shutdown_request"}))
            .is_err());
        assert!(registry
            .send_structured(
                "team",
                "team-lead",
                "worker",
                &json!({"type":"shutdown_response","request_id":"x",
                "approve":false})
            )
            .is_err());
        assert!(registry
            .send_structured(
                "team",
                "worker",
                "worker",
                &json!({"type":"plan_approval_response","request_id":"x",
                "approve":true})
            )
            .is_err());
        let plan_request = registry
            .send_structured(
                "team",
                "team-lead",
                "worker",
                &json!({"type":"plan_approval_request","request_id":"p",
                "plan_file_path":"plan.md","plan_content":"Review tests"}),
            )
            .unwrap();
        assert_eq!(plan_request["request_id"], "p");
        let plan = registry
            .send_structured(
                "team",
                "worker",
                "team-lead",
                &json!({"type":"plan_approval_response","request_id":"p",
                "approve":false,"feedback":"fix tests"}),
            )
            .unwrap();
        assert_eq!(plan["request_id"], "p");
        assert_eq!(inbox.unread("worker").unwrap().len(), 2);
    }
}

#[cfg(test)]
mod shutdown_handshake_tests {
    use super::*;

    #[test]
    fn shutdown_requires_real_request_and_is_single_use_across_registry_instances() {
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "lead").unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "alice".into(),
                    id: "alice@team".into(),
                    active: true,
                },
            )
            .unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "bob".into(),
                    id: "bob@team".into(),
                    active: true,
                },
            )
            .unwrap();
        let forged = json!({"type":"shutdown_response",
            "request_id":"shutdown-forged@alice","approve":true});
        assert!(teams
            .send_structured("team", "team-lead", "alice", &forged)
            .is_err());
        assert!(teams
            .send_structured("team", "bob", "alice", &json!({"type":"shutdown_request"}))
            .is_err());

        let first = teams
            .send_structured(
                "team",
                "alice",
                "team-lead",
                &json!({"type":"shutdown_request","reason":"finished"}),
            )
            .unwrap();
        let second = teams
            .send_structured(
                "team",
                "alice",
                "team-lead",
                &json!({"type":"shutdown_request","reason":"finished"}),
            )
            .unwrap();
        assert_ne!(first["request_id"], second["request_id"]);
        let id = first["request_id"].as_str().unwrap();
        let response = json!({"type":"shutdown_response","request_id":id,"approve":true});
        assert!(teams
            .send_structured("team", "team-lead", "bob", &response)
            .is_err());
        let other = Teams::new(root.path()).unwrap();
        assert_eq!(
            other
                .send_structured("team", "team-lead", "alice", &response)
                .unwrap()["shutdown_requested"],
            true
        );
        assert!(teams
            .send_structured("team", "team-lead", "alice", &response)
            .is_err());
        let mailbox = Mailbox::new(root.path(), "team").unwrap();
        let replies = mailbox.read("team-lead").unwrap();
        assert_eq!(replies.len(), 1);
        assert!(replies[0].text.contains("shutdown_approved"));

        teams.active("team", "bob", false).unwrap();
        assert!(teams
            .send("team", "bob", "team-lead", "hello", "note")
            .is_err());
        assert!(teams
            .send_structured(
                "team",
                "bob",
                "team-lead",
                &json!({"type":"shutdown_request"})
            )
            .is_err());
        assert_eq!(
            teams
                .send("team", "*", "team-lead", "hello", "note")
                .unwrap()["recipients"],
            json!(["alice"])
        );
    }

    #[test]
    fn shutdown_rejection_is_persisted_and_cannot_be_replayed() {
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "lead").unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "worker".into(),
                    id: "worker@team".into(),
                    active: true,
                },
            )
            .unwrap();
        let request = teams
            .send_structured(
                "team",
                "worker",
                "team-lead",
                &json!({"type":"shutdown_request"}),
            )
            .unwrap();
        let response = json!({"type":"shutdown_response",
            "request_id":request["request_id"],"approve":false,"reason":"busy"});
        assert_eq!(
            teams
                .send_structured("team", "team-lead", "worker", &response)
                .unwrap()["shutdown_requested"],
            false
        );
        assert!(teams
            .send_structured("team", "team-lead", "worker", &response)
            .is_err());
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        let replies = inbox.read("team-lead").unwrap();
        assert_eq!(replies.len(), 1);
        let payload: Value = serde_json::from_str(&replies[0].text).unwrap();
        assert_eq!(payload["type"], "shutdown_rejected");
        assert_eq!(payload["reason"], "busy");
    }
}

#[cfg(test)]
mod plan_approval_lifecycle_tests {
    use super::*;
    #[test]
    fn persisted_plan_request_approval_replay_and_restart() {
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "leader").unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "worker".into(),
                    id: "worker@team".into(),
                    active: true,
                },
            )
            .unwrap();
        let request = teams
            .send_structured(
                "team",
                "team-lead",
                "worker",
                &json!({"type":"plan_approval_request","plan_file_path":"/tmp/plan.md",
                "plan_content":"Review changes"}),
            )
            .unwrap();
        let id = request["request_id"].as_str().unwrap();
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        let pending = inbox.unread("team-lead").unwrap();
        assert_eq!(pending.len(), 1);
        let raw: Value = serde_json::from_str(&pending[0].text).unwrap();
        assert_eq!(raw["requestId"], id);
        assert_eq!(raw["planContent"], "Review changes");
        assert!(teams
            .send_structured(
                "team",
                "worker",
                "team-lead",
                &json!({"type":"plan_approval_response","request_id":"forged",
                "approve":true})
            )
            .is_err());
        assert_eq!(teams.approve_pending_plans("team", "plan").unwrap(), 1);
        let replies = inbox.unread("worker").unwrap();
        assert_eq!(replies.len(), 1);
        let approval: Value = serde_json::from_str(&replies[0].text).unwrap();
        assert_eq!(approval["requestId"], id);
        assert_eq!(approval["approved"], true);
        assert_eq!(approval["permissionMode"], "default");
        assert_eq!(
            Teams::new(root.path())
                .unwrap()
                .approve_pending_plans("team", "plan")
                .unwrap(),
            0
        );
        assert_eq!(inbox.unread("worker").unwrap().len(), 1);
        assert!(teams
            .send_structured(
                "team",
                "worker",
                "team-lead",
                &json!({"type":"plan_approval_response","request_id":id,
                "approve":false})
            )
            .is_err());
    }
    #[test]
    fn concurrent_leader_pollers_reply_once_to_each_request() {
        let root = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "leader").unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "worker".into(),
                    id: "worker@team".into(),
                    active: true,
                },
            )
            .unwrap();
        teams
            .send_structured(
                "team",
                "team-lead",
                "worker",
                &json!({"type":"plan_approval_request","plan_file_path":"plan.md",
                "plan_content":"Check"}),
            )
            .unwrap();
        let workers = (0..8)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || {
                    Teams::new(root.path())
                        .unwrap()
                        .approve_pending_plans("team", "default")
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let approvals: usize = workers.into_iter().map(|t| t.join().unwrap()).sum();
        assert_eq!(approvals, 1);
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        assert_eq!(inbox.unread("worker").unwrap().len(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn failed_plan_reply_keeps_request_pending_until_storage_recovers() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "leader").unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "worker".into(),
                    id: "worker@team".into(),
                    active: true,
                },
            )
            .unwrap();
        teams
            .send_structured(
                "team",
                "team-lead",
                "worker",
                &json!({"type":"plan_approval_request","plan_file_path":"plan.md",
                "plan_content":"Check"}),
            )
            .unwrap();
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        let worker_path = root.path().join(".atlas/teams/team/inboxes/worker.json");
        symlink(root.path().join("untrusted"), &worker_path).unwrap();
        assert!(teams.approve_pending_plans("team", "default").is_err());
        assert_eq!(inbox.unread("team-lead").unwrap().len(), 1);
        fs::remove_file(&worker_path).unwrap();
        assert_eq!(
            Teams::new(root.path())
                .unwrap()
                .approve_pending_plans("team", "default")
                .unwrap(),
            1
        );
        assert_eq!(inbox.unread("worker").unwrap().len(), 1);
    }
    #[test]
    fn mode_change_requires_active_leader_and_valid_source_mode() {
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "leader").unwrap();
        teams
            .member(
                "team",
                Member {
                    name: "worker".into(),
                    id: "worker@team".into(),
                    active: true,
                },
            )
            .unwrap();
        let request = json!({"type":"mode_set_request","mode":"acceptEdits"});
        assert!(teams
            .send_structured("team", "team-lead", "worker", &request)
            .is_err());
        assert!(teams
            .send_structured(
                "team",
                "worker",
                "team-lead",
                &json!({"type":"mode_set_request","mode":"admin"})
            )
            .is_err());
        teams
            .send_structured("team", "worker", "team-lead", &request)
            .unwrap();
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        let item = inbox.unread("worker").unwrap();
        assert_eq!(item.len(), 1);
        let payload: Value = serde_json::from_str(&item[0].text).unwrap();
        assert_eq!(payload["type"], "mode_set_request");
        assert_eq!(payload["mode"], "acceptEdits");
        assert_eq!(payload["from"], "team-lead");
    }
    #[test]
    fn plan_request_cannot_be_spoofed_or_sent_to_another_worker() {
        let root = tempfile::tempdir().unwrap();
        let teams = Teams::new(root.path()).unwrap();
        teams.create("team", None, "leader").unwrap();
        for name in ["alice", "bob"] {
            teams
                .member(
                    "team",
                    Member {
                        name: name.into(),
                        id: format!("{name}@team"),
                        active: true,
                    },
                )
                .unwrap();
        }
        let request = json!({"type":"plan_approval_request",
            "plan_file_path":"/tmp/plan","plan_content":"Work"});
        assert!(teams
            .send_structured("team", "bob", "alice", &request)
            .is_err());
        assert!(teams
            .send_structured("team", "team-lead", "team-lead", &request)
            .is_err());
        let sent = teams
            .send_structured("team", "team-lead", "alice", &request)
            .unwrap();
        let id = sent["request_id"].as_str().unwrap();
        assert!(teams
            .send_structured(
                "team",
                "bob",
                "team-lead",
                &json!({"type":"plan_approval_response","request_id":id,
                "approve":true})
            )
            .is_err());
        assert_eq!(teams.approve_pending_plans("team", "default").unwrap(), 1);
        let inbox = Mailbox::new(root.path(), "team").unwrap();
        assert!(inbox.unread("bob").unwrap().is_empty());
        assert_eq!(inbox.unread("alice").unwrap().len(), 1);
    }
}
