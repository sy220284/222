use crate::{protocol::ToolCall, session::Session};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub status: TaskStatus,
    pub notes: Option<String>,
    // Original TaskCreate/TaskUpdate persist the full description, the
    // in-progress spinner label, owner and dependency graph, not just a title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "activeForm", alias = "active_form")]
    pub active_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default)]
    pub blocks: Vec<u64>,
    #[serde(default)]
    #[serde(rename = "blockedBy", alias = "blocked_by")]
    pub blocked_by: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, Value>>,
}

pub fn handles(name: &str) -> bool {
    matches!(name, "task_create" | "task_update" | "task_list")
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"task_create",
            "description":"Create a task with a subject, description and optional active-form label and metadata.",
            "parameters":{"type":"object","properties":{
                "title":{"type":"string"},"subject":{"type":"string"},
                "description":{"type":"string"},"notes":{"type":"string"},
                "activeForm":{"type":"string"},
                "metadata":{"type":"object","additionalProperties":true}
            },"anyOf":[{"required":["subject"]},{"required":["title"]}]}}}),
        json!({"type":"function","function":{"name":"task_update",
            "description":"Update a task, assign its owner, link blockers, merge metadata, complete or delete it.",
            "parameters":{"type":"object","properties":{
                "id":{"type":"integer"},"taskId":{"type":"string"},
                "title":{"type":"string"},"subject":{"type":"string"},
                "description":{"type":"string"},"notes":{"type":"string"},
                "activeForm":{"type":"string"},"owner":{"type":"string"},
                "status":{"type":"string","enum":["pending","in_progress","completed","blocked","deleted"]},
                "addBlocks":{"type":"array","items":{"type":"string"}},
                "addBlockedBy":{"type":"array","items":{"type":"string"}},
                "metadata":{"type":"object","additionalProperties":true}
            },"anyOf":[{"required":["id"]},{"required":["taskId"]}]}}}),
        json!({"type":"function","function":{"name":"task_list",
            "description":"Read the current session task list, owners, dependencies and progress.",
            "parameters":{"type":"object","properties":{}}}}),
    ]
}

pub fn execute(session: &mut Session, call: &ToolCall) -> String {
    match dispatch(session, call) {
        Ok(value) => value,
        Err(err) => format!("ERROR: {err:#}"),
    }
}

fn task_id(value: &Value) -> Result<u64> {
    if let Some(id) = value.as_u64() {
        return Ok(id);
    }
    if let Some(id) = value.as_str() {
        return id.parse::<u64>().context("Invalid task id");
    }
    bail!("Task id must be a number or numeric string")
}

fn dependency_ids(args: &Value, key: &str) -> Result<Vec<u64>> {
    let Some(value) = args.get(key) else {
        return Ok(Vec::new());
    };
    let items = value.as_array().with_context(|| format!("{key} must be an array"))?;
    let mut ids = Vec::new();
    for value in items {
        let id = task_id(value)?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

fn nonempty_text<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let text = value.as_str().with_context(|| format!("{key} must be a string"))?;
    if text.trim().is_empty() || text.len() > 500 {
        bail!("Invalid {key}");
    }
    Ok(Some(text))
}

fn dispatch(session: &mut Session, call: &ToolCall) -> Result<String> {
    let args: Value = serde_json::from_str(&call.function.arguments)?;
    if !args.is_object() {
        bail!("Task tool arguments must be an object")
    }
    match call.function.name.as_str() {
        "task_list" => {
            // TaskListTool omits internal tasks and does not report completed
            // prerequisites as outstanding blockers.
            let completed = session.tasks.iter()
                .filter(|task| task.status == TaskStatus::Completed)
                .map(|task| task.id)
                .collect::<std::collections::HashSet<_>>();
            let mut visible = Vec::new();
            for task in &session.tasks {
                if task.metadata.as_ref()
                    .and_then(|metadata| metadata.get("_internal"))
                    .and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                let mut item = serde_json::to_value(task)?;
                item["subject"] = json!(task.title);
                item["blockedBy"] = json!(task.blocked_by.iter()
                    .filter(|id| !completed.contains(id))
                    .collect::<Vec<_>>());
                visible.push(item);
            }
            Ok(serde_json::to_string_pretty(&visible)?)
        }
        "task_create" => {
            let title = nonempty_text(&args, "subject")?
                .or(nonempty_text(&args, "title")?)
                .context("Missing task subject")?;
            let description = args.get("description").or_else(|| args.get("notes"))
                .map(|v| v.as_str().context("Task description must be a string"))
                .transpose()?;
            let active_form = args.get("activeForm")
                .map(|v| v.as_str().context("activeForm must be a string"))
                .transpose()?;
            let metadata = args.get("metadata")
                .map(|v| v.as_object().cloned().context("metadata must be an object"))
                .transpose()?;
            let id = session.tasks.iter().map(|t| t.id).max().unwrap_or(0)
                .max(session.task_id_high_water)
                .checked_add(1).context("Task id space exhausted")?;
            session.task_id_high_water = id;
            session.tasks.push(Task {
                id,
                title: title.to_owned(),
                status: TaskStatus::Pending,
                notes: args.get("notes").and_then(Value::as_str).map(str::to_owned),
                description: description.map(str::to_owned),
                active_form: active_form.map(str::to_owned),
                owner: None,
                blocks: Vec::new(),
                blocked_by: Vec::new(),
                metadata,
            });
            Ok(format!("Created task {id}: {title}"))
        }
        "task_update" => {
            let id = task_id(args.get("taskId").or_else(|| args.get("id"))
                .context("Missing task id")?)?;
            let index = session.tasks.iter().position(|task| task.id == id)
                .with_context(|| format!("No task with id {id}"))?;
            if args.get("status").and_then(Value::as_str) == Some("deleted") {
                session.task_id_high_water = session.task_id_high_water.max(id);
                session.tasks.remove(index);
                for task in &mut session.tasks {
                    task.blocks.retain(|other| *other != id);
                    task.blocked_by.retain(|other| *other != id);
                }
                return Ok(format!("Deleted task {id}"));
            }
            // Stage all changes before mutating the session so malformed
            // status/metadata/dependencies cannot partially update a task.
            let mut updated = session.tasks[index].clone();
            if let Some(title) = nonempty_text(&args, "subject")?
                .or(nonempty_text(&args, "title")?) {
                updated.title = title.to_owned();
            }
            if let Some(description) = args.get("description") {
                updated.description = Some(description.as_str()
                    .context("description must be a string")?.to_owned());
            }
            if let Some(notes) = args.get("notes") {
                updated.notes = Some(notes.as_str()
                    .context("notes must be a string")?.to_owned());
            }
            if let Some(active_form) = args.get("activeForm") {
                updated.active_form = Some(active_form.as_str()
                    .context("activeForm must be a string")?.to_owned());
            }
            if let Some(owner) = args.get("owner") {
                updated.owner = Some(owner.as_str()
                    .context("owner must be a string")?.to_owned());
            }
            if let Some(status) = args.get("status") {
                updated.status = serde_json::from_value(status.clone())
                    .context("Invalid task status")?;
            }
            if let Some(metadata) = args.get("metadata") {
                let patch = metadata.as_object().context("metadata must be an object")?;
                let target = updated.metadata.get_or_insert_with(serde_json::Map::new);
                for (key, value) in patch {
                    if value.is_null() {
                        target.remove(key);
                    } else {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
            let add_blocks = dependency_ids(&args, "addBlocks")?;
            let add_blocked_by = dependency_ids(&args, "addBlockedBy")?;
            // The original blockTask ignores missing IDs. Keep the graph
            // bidirectional for every valid link and avoid duplicates.
            let known = session.tasks.iter().map(|task| task.id)
                .collect::<std::collections::HashSet<_>>();
            for target in &add_blocks {
                if known.contains(target) && !updated.blocks.contains(target) {
                    updated.blocks.push(*target);
                }
            }
            for blocker in &add_blocked_by {
                if known.contains(blocker) && !updated.blocked_by.contains(blocker) {
                    updated.blocked_by.push(*blocker);
                }
            }
            session.tasks[index] = updated;
            for target in add_blocks {
                if let Some(other) = session.tasks.iter_mut().find(|task| task.id == target) {
                    if !other.blocked_by.contains(&id) {
                        other.blocked_by.push(id);
                    }
                }
            }
            for blocker in add_blocked_by {
                if let Some(other) = session.tasks.iter_mut().find(|task| task.id == blocker) {
                    if !other.blocks.contains(&id) {
                        other.blocks.push(id);
                    }
                }
            }
            Ok(format!("Updated task {id}"))
        }
        other => bail!("Unknown task tool {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FunctionCall;

    fn call(name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: "task".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: args.to_string(),
            },
        }
    }

    #[test]
    fn creates_updates_and_persists_tasks() {
        let root = tempfile::tempdir().unwrap();
        let mut session = Session::fresh();
        assert!(execute(
            &mut session,
            &call("task_create", json!({"title":"Run tests"}))
        )
        .contains("Created task 1"));
        assert!(execute(
            &mut session,
            &call(
                "task_update",
                json!({"id":1,"status":"completed","notes":"cargo test passed"})
            )
        )
        .contains("Updated task 1"));
        assert_eq!(session.tasks[0].status, TaskStatus::Completed);
        assert!(execute(&mut session, &call("task_list", json!({}))).contains("cargo test passed"));
        session.save(root.path()).unwrap();
        let restored = Session::load(root.path(), &session.id.to_string()).unwrap();
        assert_eq!(restored.tasks, session.tasks);
        assert!(execute(&mut session, &call("task_update", json!({"id":999}))).contains("ERROR"));
    }
}
