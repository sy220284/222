//! Workspace-local, provider-neutral task-list storage and task tool semantics.
//! Source references: utils/tasks.ts, TaskCreate/Get/List/UpdateTool.ts.
use crate::{protocol::ToolCall, task_events};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::HashSet,
    fs,
    fs::OpenOptions,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub subject: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub status: String,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

struct TaskMutationGuard {
    file: fs::File,
    deferred: Option<task_events::BatchGuard>,
}
impl Drop for TaskMutationGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
        // Dispatch observers only after the file lock is released.
        self.deferred.take();
    }
}
#[derive(Clone)]
pub struct TaskGraph {
    directory: PathBuf,
}
impl TaskGraph {
    pub fn new(root: &Path, list_id: &str) -> Result<Self> {
        if list_id.is_empty() || list_id.len() > 128 {
            bail!("Invalid task list ID")
        }
        let safe = list_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let root = root.canonicalize().context("Workspace unavailable")?;
        let base = root.join(".atlas").join("task-lists");
        fs::create_dir_all(&base)?;
        let base = base.canonicalize()?;
        if !base.starts_with(&root) {
            bail!("Task storage directory escapes workspace")
        }
        let path = base.join(safe);
        fs::create_dir_all(&path)?;
        let directory = path.canonicalize()?;
        if !directory.starts_with(&base) {
            bail!("Task list escapes workspace")
        }
        Ok(Self { directory })
    }
    fn locked<F, T>(&self, action: F) -> Result<T>
    where
        F: FnOnce(&Self) -> Result<T>,
    {
        let path = self.directory.join(".lock");
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("Unsafe task lock link")
        }
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        lock.lock_exclusive().context("Task list lock failed")?;
        let guard = TaskMutationGuard {
            file: lock,
            deferred: Some(task_events::defer_notifications()),
        };
        let result = action(self);
        drop(guard);
        result
    }
    fn valid_task_id(id: &str) -> bool {
        !id.is_empty() && id.len() <= 20 && id.bytes().all(|c| c.is_ascii_digit())
    }

    fn path(&self, id: &str) -> Result<PathBuf> {
        if !Self::valid_task_id(id) {
            bail!("Invalid task ID")
        }
        let path = self.directory.join(format!("{id}.json"));
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("Unsafe task file link")
        }
        Ok(path)
    }
    fn load(&self, id: &str) -> Result<Option<Item>> {
        // The reference getTask returns null for absent/invalid task IDs.
        // Tool inputs may contain an arbitrary string; a malformed ID must
        // not turn TaskGet, TaskUpdate or team claim into a storage error.
        if id.is_empty()
            || id.len() > 20
            || !id.bytes().all(|b| b.is_ascii_digit())
            || id.parse::<u64>().is_err()
        {
            return Ok(None);
        }
        let path = self.path(id)?;
        // Source utils/tasks.ts::getTask catches all individual file read
        // failures and returns null. A damaged, unreadable or oversized
        // record must not hide the other tasks in TaskList or block claiming.
        let bytes = match fs::read(path) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        if bytes.len() > 1_000_000 {
            return Ok(None);
        }
        // Like the reference getTask, corrupt/invalid task records are absent,
        // rather than rendering the entire shared task list unusable.
        let Ok(item) = serde_json::from_slice::<Item>(&bytes) else {
            return Ok(None);
        };
        if item.id != id
            || !matches!(
                item.status.as_str(),
                "pending" | "in_progress" | "completed"
            )
        {
            return Ok(None);
        }
        Ok(Some(item))
    }
    fn save(&self, item: &Item) -> Result<()> {
        let path = self.path(&item.id)?;
        let temp = self.directory.join(format!(".{}.tmp", Uuid::new_v4()));
        let data = serde_json::to_vec_pretty(item)?;
        if data.len() > 1_000_000 {
            bail!("Task record too large")
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        use std::io::Write;
        let result = (|| {
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            Ok::<_, anyhow::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        if result.is_ok() {
            task_events::notify_tasks_updated();
        }
        result
    }
    fn all(&self) -> Result<Vec<Item>> {
        let mut tasks = Vec::new();
        // The source listTasks treats an unavailable task directory as an
        // empty list, including when team cleanup races with a list request.
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(_) => return Ok(tasks),
        };
        for ent in entries {
            let ent = match ent {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            // A single unreadable directory entry is equivalent to a
            // missing task record in the source's listTasks/getTask chain.
            if !ent.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Corrupt or oversized task filenames must not make the entire
            // task list unreadable. The source getTask treats bad records as
            // absent, including after an interrupted write or manual edit.
            if name.is_empty()
                || name.len() > 20
                || !name.bytes().all(|c| c.is_ascii_digit())
                || name.parse::<u64>().is_err()
            {
                continue;
            }
            if let Some(item) = self.load(name)? {
                tasks.push(item)
            }
        }
        tasks.sort_by(|a, b| {
            a.id.parse::<u64>()
                .unwrap_or(0)
                .cmp(&b.id.parse::<u64>().unwrap_or(0))
        });
        Ok(tasks)
    }
    fn highest(&self) -> Result<u64> {
        let mut high = fs::read_to_string(self.directory.join(".highwatermark"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        // Preserve the highest allocated ID even when the JSON body is
        // damaged: the original findHighestTaskIdFromFiles scans filenames,
        // so a corrupt record must not allow an old task ID to be reused.
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(stem) = name.strip_suffix(".json") else {
                continue;
            };
            if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Ok(id) = stem.parse::<u64>() {
                high = high.max(id);
            }
        }
        Ok(high)
    }
    fn mark(&self, id: u64) -> Result<()> {
        // A crash during truncating write could erase the only record of
        // allocated IDs after reset. Persist the new counter completely
        // before atomically replacing the old one under the list lock.
        let target = self.directory.join(".highwatermark");
        let temp = self
            .directory
            .join(format!(".highwatermark-{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            use std::io::Write;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(id.to_string().as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, &target)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
        args.get(key)
            .and_then(Value::as_str)
            .with_context(|| format!("Missing {key}"))
    }
    fn optional_string(args: &Value, key: &str) -> Result<Option<String>> {
        match args.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            _ => bail!("Invalid {key}"),
        }
    }
    fn strict(args: &Value, allowed: &[&str]) -> Result<()> {
        let map = args
            .as_object()
            .context("Task parameters must be an object")?;
        if let Some(key) = map.keys().find(|k| !allowed.contains(&k.as_str())) {
            bail!("Unrecognized task parameter {key}")
        }
        Ok(())
    }
    fn ids(args: &Value, key: &str) -> Result<Vec<String>> {
        match args.get(key) {
            None => Ok(vec![]),
            Some(Value::Array(values)) => values
                .iter()
                .map(|v| {
                    let id = v.as_str().context("Task IDs must be strings")?;
                    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                        bail!("Invalid dependency ID")
                    }
                    Ok(id.to_owned())
                })
                .collect(),
            _ => bail!("{key} must be an array of IDs"),
        }
    }
    pub fn create(&self, args: &Value) -> Result<String> {
        Self::strict(args, &["subject", "description", "activeForm", "metadata"])?;
        let subject = Self::required(args, "subject")?;
        let description = Self::required(args, "description")?;
        let active = Self::optional_string(args, "activeForm")?;
        let metadata = match args.get("metadata") {
            None => None,
            Some(Value::Object(m)) => Some(m.clone()),
            _ => bail!("metadata must be an object"),
        };
        if subject.len() > 2000 || description.len() > 200_000 {
            bail!("Task input too large")
        }
        self.locked(|this| {
            let id = this
                .highest()?
                .checked_add(1)
                .context("Task counter exhausted")?;
            let task = Item {
                id: id.to_string(),
                subject: subject.into(),
                description: description.into(),
                active_form: active,
                owner: None,
                status: "pending".into(),
                blocks: vec![],
                blocked_by: vec![],
                metadata,
            };
            // Source createTask writes the task file under the list lock
            // and returns its ID. The high-water mark is only advanced by
            // deletion/reset; a separate mark write after successful create
            // could fail and falsely report an already-created task as failed.
            this.save(&task)?;
            Ok(format!(
                "Task #{} created successfully: {}",
                task.id, task.subject
            ))
        })
    }
    /// Read the shared task list before a teammate becomes idle.
    pub fn in_progress_owned_by(&self, owner: &str) -> Result<Vec<Item>> {
        Ok(self
            .all()?
            .into_iter()
            .filter(|task| task.status == "in_progress" && task.owner.as_deref() == Some(owner))
            .collect())
    }

    pub fn peek(&self, id: &str) -> Result<Option<Item>> {
        if !Self::valid_task_id(id) {
            return Ok(None);
        }
        self.load(id)
    }
    pub fn delete_for_blocking_hook(&self, id: &str) -> Result<bool> {
        self.locked(|this| this.delete(id))
    }
    pub fn get(&self, args: &Value) -> Result<String> {
        Self::strict(args, &["taskId"])?;
        let id = Self::required(args, "taskId")?;
        match self.peek(id)? {
            None => Ok("Task not found".into()),
            Some(task) => {
                let mut text = format!(
                    "Task #{}: {}\nStatus: {}\nDescription: {}",
                    task.id, task.subject, task.status, task.description
                );
                if !task.blocked_by.is_empty() {
                    text.push_str(&format!(
                        "\nBlocked by: {}",
                        task.blocked_by
                            .iter()
                            .map(|x| format!("#{x}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if !task.blocks.is_empty() {
                    text.push_str(&format!(
                        "\nBlocks: {}",
                        task.blocks
                            .iter()
                            .map(|x| format!("#{x}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                Ok(text)
            }
        }
    }
    pub fn list(&self, args: &Value) -> Result<String> {
        Self::strict(args, &[])?;
        let tasks = self
            .all()?
            .into_iter()
            .filter(|t| {
                // TaskListTool.ts uses !task.metadata?._internal. Metadata is
                // arbitrary JSON, so JS treats nonempty strings, nonzero
                // numbers, arrays and objects as internal too; as_bool()
                // exposed those hidden tasks in the interactive task list.
                !t.metadata
                    .as_ref()
                    .and_then(|m| m.get("_internal"))
                    .is_some_and(|value| match value {
                        Value::Null => false,
                        Value::Bool(value) => *value,
                        Value::Number(value) => value.as_f64().is_some_and(|n| n != 0.0),
                        Value::String(value) => !value.is_empty(),
                        Value::Array(_) | Value::Object(_) => true,
                    })
            })
            .collect::<Vec<_>>();
        if tasks.is_empty() {
            return Ok("No tasks found".into());
        }
        let completed = tasks
            .iter()
            .filter(|t| t.status == "completed")
            .map(|t| t.id.clone())
            .collect::<HashSet<_>>();
        Ok(tasks
            .iter()
            .map(|task| {
                let owner = task
                    .owner
                    .as_ref()
                    .map(|x| format!(" ({x})"))
                    .unwrap_or_default();
                let blockers = task
                    .blocked_by
                    .iter()
                    .filter(|id| !completed.contains(*id))
                    .map(|id| format!("#{id}"))
                    .collect::<Vec<_>>();
                let blocked = if blockers.is_empty() {
                    String::new()
                } else {
                    format!(" [blocked by {}]", blockers.join(", "))
                };
                format!(
                    "#{} [{}] {}{owner}{blocked}",
                    task.id, task.status, task.subject
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
    fn link(&self, from: &str, to: &str) -> Result<bool> {
        let (Some(mut source), Some(mut target)) = (self.load(from)?, self.load(to)?) else {
            return Ok(false);
        };
        // 2.1.88 blockTask updates both sides of a dependency. A self-link
        // addresses the same persisted task twice: writing the stale second
        // snapshot would otherwise erase its newly written blocks field.
        if from == to {
            let mut changed = false;
            if !source.blocks.iter().any(|id| id == to) {
                source.blocks.push(to.to_owned());
                changed = true;
            }
            if !source.blocked_by.iter().any(|id| id == from) {
                source.blocked_by.push(from.to_owned());
                changed = true;
            }
            if changed {
                self.save(&source)?;
            }
            return Ok(true);
        }
        if !source.blocks.contains(&to.to_owned()) {
            source.blocks.push(to.to_owned());
            self.save(&source)?;
        }
        if !target.blocked_by.contains(&from.to_owned()) {
            target.blocked_by.push(from.to_owned());
            self.save(&target)?;
        }
        Ok(true)
    }
    fn delete(&self, id: &str) -> Result<bool> {
        let path = self.path(id)?;
        if self.load(id)?.is_none() {
            return Ok(false);
        }
        let num = id.parse::<u64>()?;
        self.mark(self.highest()?.max(num))?;
        fs::remove_file(path)?;
        task_events::notify_tasks_updated();
        for mut task in self.all()? {
            let a = task.blocks.len();
            let b = task.blocked_by.len();
            task.blocks.retain(|x| x != id);
            task.blocked_by.retain(|x| x != id);
            if a != task.blocks.len() || b != task.blocked_by.len() {
                self.save(&task)?
            }
        }
        Ok(true)
    }
    /// The original task tools use strict input schemas before PreToolUse.
    /// Keep validation side-effect free so invalid task calls never invoke
    /// hooks or write task state.
    pub fn validate_tool_input(name: &str, args: &Value) -> Result<()> {
        match name {
            "TaskCreate" => {
                Self::strict(args, &["subject", "description", "activeForm", "metadata"])?;
                let subject = Self::required(args, "subject")?;
                let description = Self::required(args, "description")?;
                Self::optional_string(args, "activeForm")?;
                if args.get("metadata").is_some_and(|value| !value.is_object()) {
                    bail!("metadata must be an object")
                }
                if subject.len() > 2000 || description.len() > 200_000 {
                    bail!("Task input too large")
                }
                Ok(())
            }
            "TaskUpdate" => Self::validate_update_input(args),
            "TaskGet" => {
                Self::strict(args, &["taskId"])?;
                Self::required(args, "taskId")?;
                Ok(())
            }
            "TaskList" => Self::strict(args, &[]),
            _ => bail!("Unknown task action"),
        }
    }

    /// Validate the complete TaskUpdate input before any PreToolUse or
    /// TaskCompleted hook can run. The source Zod strict schema rejects
    /// malformed fields before invoking the tool's call method.
    pub fn validate_update_input(args: &Value) -> Result<()> {
        Self::strict(
            args,
            &[
                "taskId", "subject", "description", "activeForm", "status",
                "owner", "addBlocks", "addBlockedBy", "metadata",
            ],
        )?;
        Self::required(args, "taskId")?;
        for field in ["subject", "description", "activeForm", "owner"] {
            Self::optional_string(args, field)?;
        }
        if let Some(status) = Self::optional_string(args, "status")? {
            if !matches!(
                status.as_str(),
                "pending" | "in_progress" | "completed" | "deleted"
            ) {
                bail!("Invalid task status")
            }
        }
        Self::ids(args, "addBlocks")?;
        Self::ids(args, "addBlockedBy")?;
        if args.get("metadata").is_some_and(|value| !value.is_object()) {
            bail!("metadata must be an object")
        }
        Ok(())
    }

    pub fn update(&self, args: &Value) -> Result<String> {
        Self::validate_update_input(args)?;
        Self::strict(
            args,
            &[
                "taskId",
                "subject",
                "description",
                "activeForm",
                "status",
                "owner",
                "addBlocks",
                "addBlockedBy",
                "metadata",
            ],
        )?;
        let id = Self::required(args, "taskId")?;
        let subject = Self::optional_string(args, "subject")?;
        let description = Self::optional_string(args, "description")?;
        let active = Self::optional_string(args, "activeForm")?;
        let owner = Self::optional_string(args, "owner")?;
        let status = Self::optional_string(args, "status")?;
        let blocks = Self::ids(args, "addBlocks")?;
        let blocked_by = Self::ids(args, "addBlockedBy")?;
        let meta = match args.get("metadata") {
            None => None,
            Some(Value::Object(m)) => Some(m.clone()),
            _ => bail!("metadata must be an object"),
        };
        if let Some(state) = &status {
            if !matches!(
                state.as_str(),
                "pending" | "in_progress" | "completed" | "deleted"
            ) {
                bail!("Invalid task status")
            }
        }
        self.locked(|this| {
            let Some(mut task) = this.load(id)? else {
                return Ok("Task not found".into());
            };
            if status.as_deref() == Some("deleted") {
                return if this.delete(id)? {
                    Ok(format!("Updated task #{id} deleted"))
                } else {
                    Ok("Failed to delete task".into())
                };
            }
            let mut fields = Vec::new();
            if let Some(s) = &subject {
                if *s != task.subject {
                    task.subject = s.clone();
                    fields.push("subject")
                }
            }
            if let Some(s) = &description {
                if *s != task.description {
                    task.description = s.clone();
                    fields.push("description")
                }
            }
            if let Some(s) = &active {
                if task.active_form.as_ref() != Some(s) {
                    task.active_form = Some(s.clone());
                    fields.push("activeForm")
                }
            }
            if let Some(s) = &owner {
                if task.owner.as_ref() != Some(s) {
                    task.owner = Some(s.clone());
                    fields.push("owner")
                }
            }
            if let Some(s) = &status {
                if *s != task.status {
                    task.status = s.clone();
                    fields.push("status")
                }
            }
            if let Some(patch) = meta {
                let m = task.metadata.get_or_insert_with(Map::new);
                for (key, value) in patch {
                    if value.is_null() {
                        m.remove(&key);
                    } else {
                        m.insert(key, value);
                    }
                }
                fields.push("metadata");
            }
            if !fields.is_empty() {
                this.save(&task)?
            }
            if !blocks.is_empty() {
                let existing = task.blocks.clone();
                let new = blocks
                    .iter()
                    .filter(|x| !existing.contains(*x))
                    .collect::<Vec<_>>();
                for other in &new {
                    this.link(id, other)?;
                }
                if !new.is_empty() {
                    fields.push("blocks")
                }
            }
            if !blocked_by.is_empty() {
                let existing = task.blocked_by.clone();
                let new = blocked_by
                    .iter()
                    .filter(|x| !existing.contains(*x))
                    .collect::<Vec<_>>();
                for other in &new {
                    this.link(other, id)?;
                }
                if !new.is_empty() {
                    fields.push("blockedBy")
                }
            }
            Ok(format!("Updated task #{id} {}", fields.join(", "))
                .trim_end()
                .to_owned())
        })
    }

    /// Remove an inactive team's task storage after team cleanup succeeds.
    /// Unlike reset, this also removes the high-water mark and lock file.
    pub fn remove_storage(&self) -> Result<()> {
        if !self.directory.exists() {
            return Ok(());
        }
        self.locked(|store| {
            for entry in fs::read_dir(&store.directory)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    bail!("Unsafe task storage link")
                }
                if kind.is_dir() {
                    bail!("Unexpected nested task directory")
                }
            }
            Ok(())
        })?;
        fs::remove_dir_all(&self.directory)?;
        task_events::notify_tasks_updated();
        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        self.locked(|store| {
            // Reset for a new team removes every task JSON file, including
            // malformed or legacy records that cannot pass Item validation.
            // Record the high-water mark before deletion to prevent ID reuse
            // if an interrupted reset leaves only some files on disk.
            let high = store.highest()?;
            store.mark(high)?;
            for entry in fs::read_dir(&store.directory)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".json") && !name.starts_with('.') {
                    let kind = entry.file_type()?;
                    if kind.is_file() || kind.is_symlink() {
                        fs::remove_file(entry.path())?;
                    }
                }
            }
            task_events::notify_tasks_updated();
            Ok(())
        })
    }

    pub fn member_statuses(
        &self,
        members: &[(String, String, Option<String>)],
    ) -> Result<Vec<Value>> {
        let tasks = self.all()?;
        Ok(members
            .iter()
            .map(|(id, name, kind)| {
                let owned = tasks
                    .iter()
                    .filter(|t| {
                        t.status != "completed"
                            && (t.owner.as_deref() == Some(name) || t.owner.as_deref() == Some(id))
                    })
                    .map(|t| t.id.clone())
                    .collect::<Vec<_>>();
                json!({"agentId":id,"name":name,"agentType":kind,
                "status":if owned.is_empty(){"idle"}else{"busy"},"currentTasks":owned})
            })
            .collect())
    }

    pub fn unassign_teammate(&self, agent_id: &str, name: &str, reason: &str) -> Result<Value> {
        if !matches!(reason, "terminated" | "shutdown") {
            bail!("Invalid exit reason")
        }
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

    pub fn claim(&self, id: &str, owner: &str, check_busy: bool) -> Result<Value> {
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
    pub fn run(&self, call: &ToolCall) -> String {
        let parsed = serde_json::from_str::<Value>(&call.function.arguments);
        let result = parsed
            .and_then(|v| Ok(v))
            .map_err(anyhow::Error::from)
            .and_then(|v| match call.function.name.as_str() {
                "TaskCreate" => self.create(&v),
                "TaskGet" => self.get(&v),
                "TaskList" => self.list(&v),
                "TaskUpdate" => self.update(&v),
                _ => bail!("Unknown task action"),
            });
        match result {
            Ok(s) => s,
            Err(e) => format!("ERROR: {e:#}"),
        }
    }
}
pub fn handles(name: &str) -> bool {
    matches!(name, "TaskCreate" | "TaskGet" | "TaskList" | "TaskUpdate")
}
pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"TaskCreate","description":"Create a persistent task with subject and description.","parameters":{"type":"object","additionalProperties":false,"properties":{"subject":{"type":"string"},"description":{"type":"string"},"activeForm":{"type":"string"},"metadata":{"type":"object"}},"required":["subject","description"]}}}),
        json!({"type":"function","function":{"name":"TaskGet","description":"Read a persistent task by ID.","parameters":{"type":"object","additionalProperties":false,"properties":{"taskId":{"type":"string"}},"required":["taskId"]}}}),
        json!({"type":"function","function":{"name":"TaskList","description":"List persistent tasks, removing completed dependency blockers.","parameters":{"type":"object","additionalProperties":false,"properties":{}}}}),
        json!({"type":"function","function":{"name":"TaskUpdate","description":"Update task fields, owner, status, dependencies or metadata; delete with status deleted.","parameters":{"type":"object","additionalProperties":false,"properties":{"taskId":{"type":"string"},"subject":{"type":"string"},"description":{"type":"string"},"activeForm":{"type":"string"},"status":{"type":"string","enum":["pending","in_progress","completed","deleted"]},"owner":{"type":"string"},"addBlocks":{"type":"array","items":{"type":"string"}},"addBlockedBy":{"type":"array","items":{"type":"string"}},"metadata":{"type":"object"}},"required":["taskId"]}}}),
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> (tempfile::TempDir, TaskGraph) {
        let root = tempfile::tempdir().unwrap();
        let graph = TaskGraph::new(root.path(), "session-1").unwrap();
        (root, graph)
    }
    #[test]
    fn full_task_lifecycle_dependency_owner_metadata_and_reload() {
        let (root, g) = store();
        assert_eq!(
            g.create(&json!({"subject":"Build","description":"Build system"}))
                .unwrap(),
            "Task #1 created successfully: Build"
        );
        assert_eq!(
            g.create(&json!({"subject":"Verify","description":"Run tests","activeForm":"Testing"}))
                .unwrap(),
            "Task #2 created successfully: Verify"
        );
        assert!(g.update(&json!({"taskId":"2","addBlockedBy":["1"],"owner":"agent-A","metadata":{"revision":1,"remove":true}})).unwrap().contains("blockedBy"));
        assert!(g
            .get(&json!({"taskId":"1"}))
            .unwrap()
            .contains("Blocks: #2"));
        assert!(g.list(&json!({})).unwrap().contains("[blocked by #1]"));
        let not_claimed = g.claim("2", "agent-B", false).unwrap();
        assert_eq!(not_claimed["reason"], "already_claimed");
        let blocked = g.claim("2", "agent-A", false).unwrap();
        assert_eq!(blocked["reason"], "blocked");
        assert!(g
            .update(&json!({"taskId":"1","status":"completed"}))
            .unwrap()
            .contains("status"));
        assert!(!g.list(&json!({})).unwrap().contains("[blocked by #1]"));
        let claimed = g.claim("2", "agent-A", true).unwrap();
        assert_eq!(claimed["success"], true);
        assert!(g.update(&json!({"taskId":"2","metadata":{"remove":null,"revision":2},"status":"in_progress"})).unwrap().contains("metadata"));
        let task = g.load("2").unwrap().unwrap();
        assert_eq!(task.metadata.unwrap().get("revision"), Some(&json!(2)));
        let fresh = TaskGraph::new(root.path(), "session-1").unwrap();
        assert_eq!(fresh.all().unwrap().len(), 2);
        assert!(fresh
            .update(&json!({"taskId":"1","status":"deleted"}))
            .unwrap()
            .contains("deleted"));
        assert!(fresh
            .get(&json!({"taskId":"2"}))
            .unwrap()
            .contains("Task #2"));
        assert!(!fresh
            .get(&json!({"taskId":"2"}))
            .unwrap()
            .contains("Blocked by"));
        assert!(fresh
            .create(&json!({"subject":"Followup","description":"Next"}))
            .unwrap()
            .contains("#3"));
    }
    #[test]
    fn concurrent_creators_get_unique_monotonic_ids() {
        let (root, _g) = store();
        let root = std::sync::Arc::new(root);
        let workers = (0..12)
            .map(|i| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let graph = TaskGraph::new(root.path(), "session-1").unwrap();
                    graph
                        .create(&json!({"subject":format!("Worker {i}"),"description":"Parallel"}))
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let mut ids = workers
            .into_iter()
            .map(|t| {
                t.join()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .trim_start_matches('#')
                    .parse::<u64>()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, (1..=12).collect::<Vec<_>>());
        let graph = TaskGraph::new(root.path(), "session-1").unwrap();
        assert_eq!(graph.all().unwrap().len(), 12);
    }

    #[test]
    fn member_statuses_unassign_and_reset_highwatermark() {
        let (root, store) = store();
        store
            .create(&json!({"subject":"First","description":"Do first"}))
            .unwrap();
        store
            .create(&json!({"subject":"Second","description":"Do second"}))
            .unwrap();
        store
            .update(&json!({"taskId":"1","owner":"alice","status":"in_progress"}))
            .unwrap();
        store
            .update(&json!({"taskId":"2","owner":"agent-b","status":"completed"}))
            .unwrap();
        let members = vec![
            ("agent-a".into(), "alice".into(), Some("coder".into())),
            ("agent-b".into(), "bob".into(), None),
        ];
        let status = store.member_statuses(&members).unwrap();
        assert_eq!(status[0]["status"], "busy");
        assert_eq!(status[0]["currentTasks"], json!(["1"]));
        assert_eq!(status[1]["status"], "idle");
        let result = store
            .unassign_teammate("agent-a", "alice", "terminated")
            .unwrap();
        assert_eq!(result["unassignedTasks"][0]["id"], "1");
        assert!(result["notificationMessage"]
            .as_str()
            .unwrap()
            .contains("1 task(s) were unassigned"));
        let task = store.peek("1").unwrap().unwrap();
        assert_eq!(task.owner, None);
        assert_eq!(task.status, "pending");
        assert!(store
            .unassign_teammate("agent-a", "alice", "invalid")
            .is_err());
        store.reset().unwrap();
        assert!(store.all().unwrap().is_empty());
        let restarted = TaskGraph::new(root.path(), "session-1").unwrap();
        assert!(restarted
            .create(&json!({"subject":"Third","description":"After reset"}))
            .unwrap()
            .contains("#3"));
    }

    #[test]
    fn corrupt_task_files_are_skipped_without_hiding_valid_tasks() {
        let (_root, graph) = store();
        graph
            .create(&json!({"subject":"Valid","description":"Kept"}))
            .unwrap();
        fs::write(graph.path("2").unwrap(), "{definitely not json").unwrap();
        fs::write(graph.path("3").unwrap(),r#"{"id":"3","subject":"Bad","description":"Bad","status":"obsolete","blocks":[],"blockedBy":[]}"#).unwrap();
        assert_eq!(graph.all().unwrap().len(), 1);
        assert_eq!(graph.get(&json!({"taskId":"2"})).unwrap(), "Task not found");
        assert!(graph.list(&json!({})).unwrap().contains("Valid"));
    }
    #[test]
    fn deleting_team_storage_removes_tasks_and_counter() {
        let (_root, graph) = store();
        graph
            .create(&json!({"subject":"A","description":"B"}))
            .unwrap();
        let directory = graph.directory.clone();
        assert!(directory.join("1.json").exists());
        // Source creates the counter file on reset/deletion, not creation.
        assert!(!directory.join(".highwatermark").exists());
        graph.remove_storage().unwrap();
        assert!(!directory.exists());
    }
    #[test]
    fn subscriber_can_reenter_task_mutation_after_unlock() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let (_root, graph) = store();
        graph
            .create(&json!({"subject":"Before","description":"Body"}))
            .unwrap();
        let graph_for_callback = graph.clone();
        let once = Arc::new(AtomicBool::new(false));
        let gate = once.clone();
        let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let observed = seen.clone();
        let finished = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let completion = finished.clone();
        let _listener = task_events::on_tasks_updated(Arc::new(move || {
            let subject = graph_for_callback
                .peek("1")
                .ok()
                .flatten()
                .map(|task| task.subject)
                .unwrap_or_else(|| "<missing>".into());
            observed.lock().unwrap().push(subject.clone());
            if subject == "Original" && !gate.swap(true, Ordering::SeqCst) {
                graph_for_callback
                    .update(&json!({
                        "taskId":"1","subject":"Updated by subscriber"
                    }))
                    .unwrap();
                let (lock, ready) = &*completion;
                *lock.lock().unwrap() = true;
                ready.notify_all();
            }
        }));
        graph
            .update(&json!({"taskId":"1","subject":"Original"}))
            .unwrap();
        // A process-wide signal may also be emitted by another test thread.
        // Wait for the reentrant mutation to finish, not merely to begin.
        let (lock, ready) = &*finished;
        let (done, _) = ready
            .wait_timeout_while(
                lock.lock().unwrap(),
                std::time::Duration::from_secs(2),
                |done| !*done,
            )
            .unwrap();
        assert!(
            *done,
            "subscriber did not complete: observed={:?}",
            seen.lock().unwrap()
        );
        assert_eq!(
            graph.peek("1").unwrap().unwrap().subject,
            "Updated by subscriber",
            "observed={:?} gate={}",
            seen.lock().unwrap(),
            once.load(Ordering::SeqCst)
        );
    }
    #[test]
    fn task_mutations_emit_observable_events() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let (_root, graph) = store();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let handle = task_events::on_tasks_updated(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        graph
            .create(&json!({"subject":"A","description":"One"}))
            .unwrap();
        let after_create = hits.load(Ordering::SeqCst);
        assert!(after_create > 0);
        graph
            .update(&json!({"taskId":"1","status":"completed"}))
            .unwrap();
        let after_update = hits.load(Ordering::SeqCst);
        assert!(after_update > after_create);
        graph
            .update(&json!({"taskId":"1","status":"deleted"}))
            .unwrap();
        assert!(hits.load(Ordering::SeqCst) > after_update);
        let before_reset = hits.load(Ordering::SeqCst);
        graph.reset().unwrap();
        assert!(hits.load(Ordering::SeqCst) > before_reset);
        drop(handle);
    }

    #[test]
    fn validates_input_and_rejects_path_traversal() {
        let (root, g) = store();
        assert!(g.create(&json!({"subject":"x"})).is_err());
        assert!(g
            .create(&json!({"subject":"x","description":"ok","unknown":true}))
            .is_err());
        assert_eq!(
            g.get(&json!({"taskId":"../outside"})).unwrap(),
            "Task not found"
        );
        assert!(g.peek("../outside").unwrap().is_none());
        assert!(TaskGraph::new(root.path(), "../n").is_ok());
        assert!(g.update(&json!({"taskId":"1","status":"blocked"})).is_err());
        assert_eq!(g.get(&json!({"taskId":"999"})).unwrap(), "Task not found");
    }
    #[test]
    fn internal_tasks_hidden_and_busy_claim_fails() {
        let (_root, g) = store();
        g.create(&json!({"subject":"A","description":"desc"}))
            .unwrap();
        g.create(&json!({"subject":"B","description":"desc","metadata":{"_internal":true}}))
            .unwrap();
        g.create(&json!({"subject":"C","description":"desc"}))
            .unwrap();
        assert_eq!(g.list(&json!({})).unwrap().lines().count(), 2);
        assert_eq!(g.claim("1", "agent", true).unwrap()["success"], true);
        assert_eq!(g.claim("3", "agent", true).unwrap()["reason"], "agent_busy");
    }
}
