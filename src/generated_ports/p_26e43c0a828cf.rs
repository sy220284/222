//! Source-aligned export dialog state and terminal view.
//! The original component has two mutually exclusive screens: export-method
//! selection and filename input. Keep their transitions observable so a host
//! can route the corresponding terminal events without inventing callbacks.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::{fs::OpenOptions, io::Write, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDialogState {
    pub content: String,
    pub filename: String,
    pub cursor_offset: usize,
    pub show_filename_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportAction {
    /// The terminal host performs the clipboard operation before acknowledging
    /// completion; source `setClipboard` can emit an OSC payload to stdout.
    Clipboard(String),
    Done {
        success: bool,
        message: String,
    },
}

impl ExportDialogState {
    pub fn new(content: impl Into<String>, default_filename: impl Into<String>) -> Self {
        let filename = default_filename.into();
        Self {
            content: content.into(),
            cursor_offset: filename.encode_utf16().count(),
            filename,
            show_filename_input: false,
        }
    }

    pub fn select_option(&mut self, value: &str) -> Option<ExportAction> {
        match value {
            "clipboard" => Some(ExportAction::Clipboard(self.content.clone())),
            "file" => {
                self.show_filename_input = true;
                None
            }
            _ => None,
        }
    }

    pub fn cancel(&mut self) -> Option<ExportAction> {
        if self.show_filename_input {
            self.show_filename_input = false;
            None
        } else {
            Some(ExportAction::Done {
                success: false,
                message: "Export cancelled".into(),
            })
        }
    }

    pub fn update_filename(&mut self, value: impl Into<String>) {
        self.filename = value.into();
    }

    pub fn update_cursor_offset(&mut self, cursor_offset: usize) {
        self.cursor_offset = cursor_offset;
    }

    /// Matches the original `.endsWith('.txt') ? filename :
    /// filename.replace(/\.[^.]+$/, '') + '.txt'` behavior.
    pub fn final_filename(&self) -> String {
        if self.filename.ends_with(".txt") {
            return self.filename.clone();
        }
        let stem = self
            .filename
            .rfind('.')
            .filter(|&i| i + 1 < self.filename.len());
        format!(
            "{}.txt",
            stem.map_or(self.filename.as_str(), |i| &self.filename[..i])
        )
    }

    pub fn submit_file(&self, cwd: &Path) -> ExportAction {
        let path = cwd.join(self.final_filename());
        let result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)?;
            file.write_all(self.content.as_bytes())?;
            file.sync_all()
        })();
        match result {
            Ok(()) => ExportAction::Done {
                success: true,
                message: format!("Conversation exported to: {}", path.display()),
            },
            Err(error) => ExportAction::Done {
                success: false,
                message: format!("Failed to export conversation: {error}"),
            },
        }
    }

    pub fn clipboard_completed(&self) -> ExportAction {
        ExportAction::Done {
            success: true,
            message: "Conversation copied to clipboard".into(),
        }
    }
}

fn node(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    let attrs = attrs
        .as_object()
        .expect("view attributes are JSON objects")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}

fn input_guide(editing: bool, exit_state: Option<&Value>) -> VNode {
    if editing {
        return node(
            "Byline",
            json!({}),
            vec![
                node(
                    "KeyboardShortcutHint",
                    json!({"shortcut":"Enter","action":"save"}),
                    vec![],
                ),
                node(
                    "ConfigurableShortcutHint",
                    json!({"action":"confirm:no","context":"Confirmation","fallback":"Esc","description":"go back"}),
                    vec![],
                ),
            ],
        );
    }
    if let Some(exit_state) = exit_state.filter(|state| state["pending"] == true) {
        let name = exit_state["keyName"].as_str().unwrap_or("Ctrl-C");
        return node(
            "Text",
            json!({}),
            vec![VNode::Text(format!("Press {name} again to exit"))],
        );
    }
    node(
        "ConfigurableShortcutHint",
        json!({"action":"confirm:no","context":"Confirmation","fallback":"Esc","description":"cancel"}),
        vec![],
    )
}

/// Render from a resolved host state snapshot. Event names map onto the
/// ExportDialogState transitions, rather than serializing missing JS closures.
pub fn render_source_view(props: &Value) -> VNode {
    let filename = props["filename"]
        .as_str()
        .or_else(|| props["defaultFilename"].as_str())
        .unwrap_or_default();
    let editing = props["showFilenameInput"] == true;
    let cursor_offset = props["cursorOffset"]
        .as_u64()
        .unwrap_or_else(|| filename.encode_utf16().count() as u64);
    let guide = input_guide(editing, props.get("exitState"));
    let child = if editing {
        node(
            "Box",
            json!({"flexDirection":"column"}),
            vec![
                node(
                    "Text",
                    json!({}),
                    vec![VNode::Text("Enter filename:".into())],
                ),
                node(
                    "Box",
                    json!({"flexDirection":"row","gap":1,"marginTop":1}),
                    vec![
                        node("Text", json!({}), vec![VNode::Text(">".into())]),
                        node(
                            "TextInput",
                            json!({
                                "value":filename,"focus":true,"showCursor":true,
                                "columns":props["columns"],"cursorOffset":cursor_offset,
                                "onChange":"export.update_filename",
                                "onSubmit":"export.submit_file",
                                "onChangeCursorOffset":"export.update_cursor_offset"
                            }),
                            vec![],
                        ),
                    ],
                ),
            ],
        )
    } else {
        node(
            "Select",
            json!({
                "options":[
                    {"label":"Copy to clipboard","value":"clipboard","description":"Copy the conversation to your system clipboard"},
                    {"label":"Save to file","value":"file","description":"Save the conversation to a file in the current directory"}
                ],
                "onChange":"export.select_option","onCancel":"export.cancel"
            }),
            vec![],
        )
    };
    node(
        "Dialog",
        json!({
            "title":"Export Conversation",
            "subtitle":"Select export method:",
            "color":"permission",
            "onCancel":"export.cancel",
            "inputGuide":serde_json::to_value(guide).expect("view tree serializes"),
            "isCancelActive":!editing
        }),
        vec![child],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_cancel_and_clipboard_follow_source_transitions() {
        let mut state = ExportDialogState::new("hello", "draft.md");
        assert_eq!(state.cursor_offset, 8);
        assert_eq!(state.select_option("unknown"), None);
        assert_eq!(
            state.select_option("clipboard"),
            Some(ExportAction::Clipboard("hello".into()))
        );
        assert_eq!(
            state.clipboard_completed(),
            ExportAction::Done {
                success: true,
                message: "Conversation copied to clipboard".into()
            }
        );
        assert_eq!(state.select_option("file"), None);
        assert!(state.show_filename_input);
        assert_eq!(state.cancel(), None);
        assert!(!state.show_filename_input);
        assert_eq!(
            state.cancel(),
            Some(ExportAction::Done {
                success: false,
                message: "Export cancelled".into()
            })
        );
    }
    #[test]
    fn file_export_writes_bytes_and_reports_actual_io_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = ExportDialogState::new("α\n🙂", "draft.md");
        assert_eq!(state.final_filename(), "draft.txt");
        state.update_filename("报告.txt");
        assert_eq!(state.final_filename(), "报告.txt");
        let path = dir.path().join("报告.txt");
        assert_eq!(
            state.submit_file(dir.path()),
            ExportAction::Done {
                success: true,
                message: format!("Conversation exported to: {}", path.display())
            }
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "α\n🙂");
        state.update_filename("nonexistent/subdir.txt");
        assert!(
            matches!(state.submit_file(dir.path()), ExportAction::Done {success: false, message} if message.starts_with("Failed to export conversation:"))
        );
    }
    #[test]
    fn both_view_states_preserve_options_input_and_exit_guides() {
        let select = render_source_view(
            &json!({"defaultFilename":"draft.md", "exitState":{"pending":true,"keyName":"Ctrl-C"}}),
        );
        let raw = serde_json::to_value(&select).unwrap();
        assert_eq!(raw["children"][0]["tag"], "Select");
        assert_eq!(
            raw["children"][0]["attrs"]["options"][0]["value"],
            "clipboard"
        );
        assert_eq!(
            raw["attrs"]["inputGuide"]["children"][0]["value"],
            "Press Ctrl-C again to exit"
        );
        let edit = render_source_view(
            &json!({"showFilenameInput":true,"filename":"report.txt","columns":80,"cursorOffset":4}),
        );
        let raw = serde_json::to_value(&edit).unwrap();
        assert_eq!(raw["attrs"]["isCancelActive"], false);
        assert_eq!(raw["attrs"]["inputGuide"]["tag"], "Byline");
        assert_eq!(
            raw["children"][0]["children"][1]["children"][1]["attrs"]["value"],
            "report.txt"
        );
        assert_eq!(edit.unresolved_node_count(), 0);
    }
}
