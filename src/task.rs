use crate::{protocol::ToolCall, session::Session};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Legacy non-interactive TodoWrite items (2.1.88 utils/todo/types.ts).
/// Stored with the session so SDK/headless requests can resume their checklist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub active_form: String,
}

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
    matches!(
        name,
        "task_create" | "task_update" | "task_list" | crate::tool_names::TODO_WRITE_TOOL_NAME | "todo_write"
    )
}

/// The original enables TaskCreate/Get/List/Update in interactive sessions
/// and TodoWrite in non-interactive sessions, unless tasks are explicitly
/// enabled. Use a provider-neutral opt-in name for SDK clients.
pub fn tasks_v2_enabled(interactive: bool) -> bool {
    interactive
        || std::env::var("ATLAS_ENABLE_TASKS")
            .ok()
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
}

pub fn todo_definition() -> Value {
    json!({"type":"function","function":{
        "name": crate::tool_names::TODO_WRITE_TOOL_NAME,
        "description":"Replace the session task checklist with pending, in-progress and completed items.",
        "parameters":{"type":"object","additionalProperties":false,
            "properties":{"todos":{"type":"array","items":{"type":"object",
                "properties":{
                    "content":{"type":"string","minLength":1},
                    "status":{"type":"string","enum":["pending","in_progress","completed"]},
                    "activeForm":{"type":"string","minLength":1}
                },
                "required":["content","status","activeForm"]}}},
            "required":["todos"]}
    }})
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
        crate::tool_names::TODO_WRITE_TOOL_NAME | "todo_write" => {
            let object = args
                .as_object()
                .context("TodoWrite arguments must be an object")?;
            if object.len() != 1 || !object.contains_key("todos") {
                bail!("TodoWrite accepts only the todos field")
            }
            let input = object["todos"]
                .as_array()
                .context("todos must be an array")?;
            let mut todos = Vec::with_capacity(input.len());
            for (index, value) in input.iter().enumerate() {
                let item: TodoItem = serde_json::from_value(value.clone())
                    .with_context(|| format!("Invalid todo item {index}"))?;
                if item.content.is_empty() || item.active_form.is_empty() {
                    bail!("Todo content and activeForm must not be empty")
                }
                if !matches!(
                    item.status.as_str(),
                    "pending" | "in_progress" | "completed"
                ) {
                    bail!("Invalid todo status at index {index}")
                }
                todos.push(item);
            }
            // TodoWriteTool clears the stored list after the final item is
            // completed, while returning a successful tool result.
            session.todos = if todos.iter().all(|item| item.status == "completed") {
                Vec::new()
            } else {
                todos
            };
            Ok("Todos have been modified successfully. Ensure that you continue to use the todo list to track your progress. Please proceed with the current tasks if applicable".to_owned())
        }
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
            // The source TaskUpdate schema validates every supplied field
            // before updateTask mutates the stored record. Validate the full
            // request first so a bad status cannot leave a changed title.
            let title = args
                .get("title")
                .map(|value| value.as_str().context("Invalid task title"))
                .transpose()?;
            if title.is_some_and(|value| value.trim().is_empty() || value.len() > 500) {
                bail!("Invalid task title")
            }
            let status: Option<TaskStatus> = args
                .get("status")
                .map(|value| serde_json::from_value(value.clone()).context("Invalid task status"))
                .transpose()?;
            let notes = args
                .get("notes")
                .map(|value| value.as_str().context("Invalid task notes"))
                .transpose()?;
            let task = session
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .with_context(|| format!("No task with id {id}"))?;
            if let Some(title) = title {
                task.title = title.to_owned();
            }
            if let Some(status) = status {
                task.status = status;
            }
            if let Some(notes) = notes {
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
