use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::Path,
};
use uuid::Uuid;

/// An in-process read revision. It detects both external modifications and
/// repeated edits before the agent has read the updated notebook.
pub fn revision(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

fn required<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .with_context(|| format!("NotebookEdit missing {name}"))
}

fn select_cell(cells: &[Value], id: &str) -> Result<usize> {
    if let Some(index) = cells
        .iter()
        .position(|c| c.get("id").and_then(Value::as_str) == Some(id))
    {
        return Ok(index);
    }
    let index = id
        .strip_prefix("cell-")
        .and_then(|text| text.parse::<usize>().ok())
        .with_context(|| format!("Cell ID {id} not found"))?;
    if index >= cells.len() {
        bail!("Cell index {index} outside notebook")
    }
    Ok(index)
}

pub fn edit(path: &Path, args: &Value, known_revision: Option<u64>) -> Result<(String, u64)> {
    if path.extension().and_then(|e| e.to_str()) != Some("ipynb") {
        bail!("NotebookEdit requires an .ipynb path")
    }
    let old = fs::read_to_string(path)?;
    if old.len() > 8_000_000 {
        bail!("Notebook too large")
    }
    if Some(revision(&old)) != known_revision {
        bail!("Read this notebook before editing; the file may have changed")
    }
    let mut data: Value = serde_json::from_str(&old).context("Notebook must be valid JSON")?;
    let notebook = data
        .as_object_mut()
        .context("Notebook root must be a JSON object")?;
    let major = notebook
        .get("nbformat")
        .and_then(Value::as_u64)
        .unwrap_or(4);
    let minor = notebook
        .get("nbformat_minor")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let has_cell_ids = major > 4 || (major == 4 && minor >= 5);
    let language = notebook
        .get("metadata")
        .and_then(|m| m.pointer("/language_info/name"))
        .and_then(Value::as_str)
        .unwrap_or("python")
        .to_owned();
    let cells = notebook
        .get_mut("cells")
        .and_then(Value::as_array_mut)
        .context("Notebook missing cells array")?;
    let mode = args
        .get("edit_mode")
        .and_then(Value::as_str)
        .unwrap_or("replace");
    if !matches!(mode, "replace" | "insert" | "delete") {
        bail!("Invalid notebook edit mode {mode}")
    }
    let id = args.get("cell_id").and_then(Value::as_str);
    let source = required(args, "new_source")?;
    let requested_type = args.get("cell_type").and_then(Value::as_str);
    if requested_type.is_some_and(|t| !matches!(t, "code" | "markdown")) {
        bail!("Unsupported notebook cell type")
    }
    if mode == "insert" && requested_type.is_none() {
        bail!("cell_type is required for notebook insertion")
    }
    if mode != "insert" && id.is_none() {
        bail!("cell_id is required to edit or delete a notebook cell")
    }
    let index = match id {
        Some(id) => select_cell(cells, id)?,
        None => 0,
    };
    let mut cell_id = id.map(str::to_owned);
    let mut final_type = requested_type.unwrap_or("code").to_owned();
    match mode {
        "insert" => {
            let insert_at = if id.is_some() { index + 1 } else { 0 };
            let id_value = if has_cell_ids {
                let generated = Uuid::new_v4().simple().to_string()[..13].to_owned();
                cell_id = Some(generated.clone());
                Some(generated)
            } else {
                None
            };
            let mut cell = json!({
                "cell_type": final_type,
                "metadata": {},
                "source": source,
            });
            if let Some(value) = id_value {
                cell["id"] = json!(value);
            }
            if final_type == "code" {
                cell["execution_count"] = Value::Null;
                cell["outputs"] = json!([]);
            }
            cells.insert(insert_at, cell);
        }
        "replace" => {
            let cell = cells.get_mut(index).context("Notebook cell missing")?;
            final_type = requested_type
                .unwrap_or_else(|| {
                    cell.get("cell_type")
                        .and_then(Value::as_str)
                        .unwrap_or("code")
                })
                .to_owned();
            cell["source"] = json!(source);
            cell["cell_type"] = json!(final_type);
            if final_type == "code" {
                cell["execution_count"] = Value::Null;
                cell["outputs"] = json!([]);
            } else {
                cell.as_object_mut()
                    .context("Cell must be object")?
                    .remove("execution_count");
                cell.as_object_mut()
                    .context("Cell must be object")?
                    .remove("outputs");
            }
            cell_id = cell
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or(cell_id);
        }
        "delete" => {
            final_type = cells[index]
                .get("cell_type")
                .and_then(Value::as_str)
                .unwrap_or("code")
                .to_owned();
            cells.remove(index);
        }
        _ => unreachable!(),
    }
    let updated = serde_json::to_string_pretty(&data)?;
    if updated.len() > 8_000_000 {
        bail!("Updated notebook too large")
    }
    let temporary = path.with_extension(format!("ipynb.{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, &updated)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    let summary = json!({
        "notebook_path": path.display().to_string(),
        "edit_mode": mode,
        "cell_id": cell_id,
        "cell_type": final_type,
        "language": language,
        "new_source": source,
    });
    Ok((summary.to_string(), revision(&updated)))
}

pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"notebook_edit",
        "description":format!(
            "{}\nThis implementation supports insert, replace, delete and cell-N indices. A previously read notebook and permission approval are required.",
            crate::source_prompt_catalog::literal(
                "src/tools/NotebookEditTool/prompt.ts",
                "Completely replaces the contents of a specific cell",
            )
            .expect("Original NotebookEdit tool prompt must exist")
        ),
        "parameters":{"type":"object","properties":{
            "notebook_path":{"type":"string"},
            "cell_id":{"type":"string"},
            "new_source":{"type":"string"},
            "cell_type":{"type":"string","enum":["code","markdown"]},
            "edit_mode":{"type":"string","enum":["insert","replace","delete"]}
        },"required":["notebook_path","new_source"]}
    }})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notebook_insert_replace_delete_and_stale_revision() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("a.ipynb");
        fs::write(
            &path,
            json!({"nbformat":4,"nbformat_minor":5,
            "metadata":{"language_info":{"name":"python"}},
            "cells":[{"id":"abc","cell_type":"code","source":["print(1)\n"],
                "outputs":[{"text":"old"}],"execution_count":2,"metadata":{}}]})
            .to_string(),
        )
        .unwrap();
        let original = fs::read_to_string(&path).unwrap();
        let (result, _) = edit(
            &path,
            &json!({"cell_id":"abc","new_source":"print(2)",
            "edit_mode":"replace"}),
            Some(revision(&original)),
        )
        .unwrap();
        assert!(result.contains("replace"));
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["cells"][0]["source"], "print(2)");
        assert_eq!(doc["cells"][0]["outputs"], json!([]));
        assert!(doc["cells"][0]["execution_count"].is_null());
        assert!(edit(
            &path,
            &json!({"cell_id":"abc","new_source":"wrong"}),
            Some(revision(&original))
        )
        .is_err());
        let revised = fs::read_to_string(&path).unwrap();
        let (inserted, _) = edit(
            &path,
            &json!({"edit_mode":"insert","new_source":"# Heading",
            "cell_type":"markdown"}),
            Some(revision(&revised)),
        )
        .unwrap();
        assert!(inserted.contains("insert"));
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["cells"][0]["cell_type"], "markdown");
        let id = doc["cells"][0]["id"].as_str().unwrap();
        assert_eq!(id.len(), 13);
        let revised = fs::read_to_string(&path).unwrap();
        edit(
            &path,
            &json!({"edit_mode":"delete","new_source":"","cell_id":"cell-0"}),
            Some(revision(&revised)),
        )
        .unwrap();
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["cells"].as_array().unwrap().len(), 1);
    }
}
