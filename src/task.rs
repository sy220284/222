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
}

pub fn handles(name: &str) -> bool {
    matches!(name, "task_create" | "task_update" | "task_list")
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"task_create",
            "description":"Add a new trackable task to the current session.",
            "parameters":{"type":"object","properties":{
                "title":{"type":"string"},"notes":{"type":"string"}
            },"required":["title"]}}}),
        json!({"type":"function","function":{"name":"task_update",
            "description":"Update a task's title, status or notes. Only mark completed after verifying the work.",
            "parameters":{"type":"object","properties":{
                "id":{"type":"integer"},"title":{"type":"string"},
                "status":{"type":"string","enum":["pending","in_progress","completed","blocked"]},
                "notes":{"type":"string"}
            },"required":["id"]}}}),
        json!({"type":"function","function":{"name":"task_list",
            "description":"Read all tasks and their current status in this session.",
            "parameters":{"type":"object","properties":{}}}}),
    ]
}

pub fn execute(session: &mut Session, call: &ToolCall) -> String {
    match dispatch(session, call) {
        Ok(value) => value,
        Err(err) => format!("ERROR: {err:#}"),
    }
}

fn dispatch(session: &mut Session, call: &ToolCall) -> Result<String> {
    let args: Value = serde_json::from_str(&call.function.arguments)?;
    if !args.is_object() {
        bail!("Task tool arguments must be an object")
    }
    match call.function.name.as_str() {
        "task_list" => Ok(serde_json::to_string_pretty(&session.tasks)?),
        "task_create" => {
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .context("Missing title")?;
            if title.trim().is_empty() {
                bail!("Task title cannot be empty")
            }
            if title.len() > 500 {
                bail!("Task title is too long")
            }
            let id = session.tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            session.tasks.push(Task {
                id,
                title: title.to_owned(),
                status: TaskStatus::Pending,
                notes: args.get("notes").and_then(Value::as_str).map(str::to_owned),
            });
            Ok(format!("Created task {id}: {title}"))
        }
        "task_update" => {
            let id = args
                .get("id")
                .and_then(Value::as_u64)
                .context("Missing task id")?;
            let task = session
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .with_context(|| format!("No task with id {id}"))?;
            if let Some(title) = args.get("title").and_then(Value::as_str) {
                if title.trim().is_empty() || title.len() > 500 {
                    bail!("Invalid task title")
                }
                task.title = title.to_owned();
            }
            if let Some(status) = args.get("status") {
                task.status =
                    serde_json::from_value(status.clone()).context("Invalid task status")?;
            }
            if let Some(notes) = args.get("notes").and_then(Value::as_str) {
                task.notes = Some(notes.to_owned());
            }
            Ok(format!("Updated task {}", task.id))
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
