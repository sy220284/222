//! Files currently referenced by successful read-file tool calls in the
//! active conversation. A conservative reconstructable counterpart of the
//! original readFileState cache (which is not yet persisted independently).
use crate::protocol::Message;
use std::{collections::{BTreeMap, HashMap}, path::{Path, PathBuf}};

fn read_target(name: &str, args: &str) -> Option<String> {
    if !matches!(name, "read_file" | "FileRead" | "Read") {
        return None;
    }
    let input: serde_json::Value=serde_json::from_str(args).ok()?;
    input.get("path")
        .or_else(|| input.get("file_path"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn paths(messages: &[Message], root: &Path) -> Vec<String> {
    let mut pending: HashMap<&str,String> = HashMap::new();
    let mut found = BTreeMap::<String,String>::new();
    for message in messages {
        if message.role == "assistant" {
            if let Some(calls) = message.tool_calls.as_ref() {
                for call in calls {
                    if let Some(path) = read_target(&call.function.name, &call.function.arguments) {
                        pending.insert(&call.id, path);
                    }
                }
            }
        } else if message.role == "tool" {
            if let Some(call_id) = message.tool_call_id.as_deref() {
                if let Some(path) = pending.remove(call_id) {
                    if message.content.as_deref().is_some_and(|s| s.starts_with("ERROR:")) {
                        continue;
                    }
                    let absolute=if Path::new(&path).is_absolute() {
                        PathBuf::from(path)
                    } else {
                        root.join(path)
                    };
                    if let Ok(canonical)=absolute.canonicalize() {
                        if canonical.is_file() {
                            let display=canonical.strip_prefix(root).unwrap_or(&canonical)
                                .to_string_lossy().replace('\\',"/");
                            found.insert(canonical.to_string_lossy().to_string(),display);
                        }
                    }
                }
            }
        }
    }
    found.into_values().collect()
}

pub fn render(messages: &[Message], root: &Path) -> String {
    let files = paths(messages,root);
    if files.is_empty() {
        "No files in context".to_owned()
    } else {
        format!("Files in context:\n{}",files.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FunctionCall,ToolCall};
    fn read(id: &str,path: &str) -> Message {
        let call=ToolCall {
            id:id.to_owned(),kind:"function".into(),
            function:FunctionCall{name:"read_file".into(),
                arguments:serde_json::json!({"path":path}).to_string()},
        };
        Message{
            role:"assistant".into(),content:None,content_blocks:None,
            tool_calls:Some(vec![call]),tool_call_id:None
        }
    }
    fn reply(id:&str,text:&str)->Message {
        Message {
            role:"tool".into(),content:Some(text.into()),content_blocks:None,
            tool_calls:None,tool_call_id:Some(id.into())
        }
    }
    #[test]
    fn only_completed_successful_file_reads_are_reported_once() {
        let root=tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.rs"),"test").unwrap();
        std::fs::write(root.path().join("b.rs"),"test").unwrap();
        let messages=vec![
            read("a","a.rs"), read("b","missing.rs"),read("c","b.rs"),
            reply("b","ERROR: denied"), reply("c","contents"),
            reply("a","contents"),read("d","a.rs"),reply("d","contents"),
        ];
        assert_eq!(paths(&messages,root.path()),vec!["a.rs","b.rs"]);
        assert!(render(&messages,root.path()).contains("Files in context:\na.rs\nb.rs"));
    }
    #[test]
    fn no_read_returns_original_empty_message() {
        let root=tempfile::tempdir().unwrap();
        assert_eq!(render(&[],root.path()),"No files in context");
        assert!(paths(&[read("unknown","/nonexistent")],root.path()).is_empty());
    }
}
