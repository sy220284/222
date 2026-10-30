//! Ordered user/notification queue, matching src/utils/messageQueueManager.ts.
//! The queue is local to a session; it never contacts an external service.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Now,
    Next,
    Later,
}
impl Priority {
    fn rank(self) -> u8 {
        match self {
            Self::Now => 0,
            Self::Next => 1,
            Self::Later => 2,
        }
    }
}
impl Default for Priority {
    fn default() -> Self {
        Self::Next
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Bash,
    Prompt,
    OrphanedPermission,
    TaskNotification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub value: Value,
    pub mode: Mode,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orphaned_permission: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pasted_contents: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_expansion_value: Option<String>,
    #[serde(default)]
    pub skip_slash_commands: bool,
    #[serde(default)]
    pub bridge_origin: bool,
    #[serde(default)]
    pub is_meta: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

impl Command {
    pub fn priority(&self) -> Priority {
        self.priority.unwrap_or_default()
    }
    pub fn is_editable(&self) -> bool {
        self.mode != Mode::TaskNotification && !self.is_meta
    }
    pub fn is_visible(&self, channels_enabled: bool) -> bool {
        if channels_enabled
            && self
                .origin
                .as_ref()
                .and_then(|origin| origin.get("kind"))
                .and_then(Value::as_str)
                == Some("channel")
        {
            return true;
        }
        self.is_editable()
    }
    pub fn is_slash(&self) -> bool {
        // processUserInput.ts takes slash commands from the final text block,
        // including queued user turns containing earlier image blocks.
        let text = self.value.as_str().or_else(|| {
            let last = self.value.as_array()?.last()?;
            (last.get("type").and_then(Value::as_str) == Some("text"))
                .then(|| last.get("text").and_then(Value::as_str))
                .flatten()
        });
        text.is_some_and(|value| value.trim().starts_with('/')) && !self.skip_slash_commands
    }
    pub fn text(&self) -> Option<String> {
        if let Some(text) = self.value.as_str() {
            return Some(text.to_owned());
        }
        let blocks = self.value.as_array()?;
        let mut segments = Vec::new();
        for block in blocks {
            if block.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    segments.push(text);
                }
            }
        }
        Some(segments.join("\n"))
    }
    pub fn contains_image(&self) -> bool {
        self.value.as_array().is_some_and(|blocks| {
            blocks
                .iter()
                .any(|v| v.get("type").and_then(Value::as_str) == Some("image"))
        }) || self
            .pasted_contents
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|map| {
                map.values()
                    .any(|v| v.get("type").and_then(Value::as_str) == Some("image"))
            })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub identity: u64,
    #[serde(flatten)]
    pub command: Command,
}

/// Snapshot is a clone, not a live mutable reference; revisions are updated
/// only when the queue actually mutates (plus explicit recheck).
#[derive(Default)]
pub struct Queue {
    items: Vec<Item>,
    next_identity: u64,
    revision: u64,
    listeners: Vec<(u64, Box<dyn Fn()>)>,
    next_listener: u64,
    journal: Option<crate::queue_journal::Journal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PopEditable {
    pub text: String,
    pub cursor_offset: usize,
    pub images: Vec<Value>,
}

impl Queue {
    pub fn enable_journal(&mut self, root: &std::path::Path, session: &str) -> anyhow::Result<()> {
        self.journal = Some(crate::queue_journal::Journal::new(root, session)?);
        Ok(())
    }
    fn record(&self, operation: &str, content: Option<&str>) {
        if let Some(journal) = &self.journal {
            if let Err(error) = journal.record(operation, content) {
                eprintln!("Queue operation journal failed: {error:#}");
            }
        }
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn snapshot(&self) -> Vec<Item> {
        self.items.clone()
    }
    fn notify(&mut self) {
        self.revision = self.revision.saturating_add(1);
        for (_, listener) in &self.listeners {
            listener()
        }
    }
    pub fn subscribe(&mut self, listener: impl Fn() + 'static) -> u64 {
        self.next_listener += 1;
        let id = self.next_listener;
        self.listeners.push((id, Box::new(listener)));
        id
    }
    pub fn unsubscribe(&mut self, id: u64) {
        self.listeners.retain(|(current, _)| *current != id);
    }
    pub fn enqueue(&mut self, mut command: Command, notification: bool) -> u64 {
        if command.priority.is_none() {
            command.priority = Some(if notification {
                Priority::Later
            } else {
                Priority::Next
            });
        }
        self.next_identity += 1;
        let identity = self.next_identity;
        self.items.push(Item { identity, command });
        self.notify();
        self.record(
            "enqueue",
            self.items
                .last()
                .and_then(|item| item.command.value.as_str()),
        );
        identity
    }
    fn best_index(&self, filter: impl Fn(&Command) -> bool) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| filter(&item.command))
            .min_by_key(|(_, item)| item.command.priority().rank())
            .map(|(i, _)| i)
    }
    pub fn peek(&self, filter: impl Fn(&Command) -> bool) -> Option<&Item> {
        self.best_index(filter).map(|i| &self.items[i])
    }
    pub fn dequeue(&mut self, filter: impl Fn(&Command) -> bool) -> Option<Item> {
        let index = self.best_index(filter)?;
        let result = self.items.remove(index);
        self.notify();
        self.record("dequeue", None);
        Some(result)
    }
    /// Unlike dequeue(), original dequeueAll() returns raw insertion order.
    pub fn dequeue_all(&mut self) -> Vec<Item> {
        if self.items.is_empty() {
            return Vec::new();
        }
        let items = std::mem::take(&mut self.items);
        self.notify();
        for _ in &items {
            self.record("dequeue", None)
        }
        items
    }
    /// Original dequeueAllMatching also preserves insertion order, *not*
    /// priority order, despite the comment in the TypeScript source.
    pub fn dequeue_all_matching(&mut self, filter: impl Fn(&Command) -> bool) -> Vec<Item> {
        self.remove_filtered(filter, "dequeue")
    }
    pub fn remove_matching(&mut self, filter: impl Fn(&Command) -> bool) -> Vec<Item> {
        self.remove_filtered(filter, "remove")
    }
    fn remove_filtered(&mut self, filter: impl Fn(&Command) -> bool, operation: &str) -> Vec<Item> {
        let mut removed = Vec::new();
        self.items.retain(|item| {
            if filter(&item.command) {
                removed.push(item.clone());
                false
            } else {
                true
            }
        });
        if !removed.is_empty() {
            self.notify();
            for _ in &removed {
                self.record(operation, None)
            }
        }
        removed
    }
    pub fn remove_identity(&mut self, identities: &[u64]) -> usize {
        let before = self.items.len();
        self.items
            .retain(|item| !identities.contains(&item.identity));
        let removed = before - self.items.len();
        if removed > 0 {
            self.notify();
            for _ in 0..removed {
                self.record("remove", None)
            }
        }
        removed
    }
    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.notify()
        }
    }
    pub fn reset(&mut self) {
        self.items.clear();
        self.revision = 0;
        self.next_identity = 0;
    }
    pub fn recheck(&mut self) {
        if !self.items.is_empty() {
            self.notify()
        }
    }
    pub fn by_max_priority(&self, max: Priority) -> Vec<Item> {
        self.items
            .iter()
            .filter(|item| item.command.priority().rank() <= max.rank())
            .cloned()
            .collect()
    }
    pub fn pop_all_editable(
        &mut self,
        current_input: &str,
        cursor_offset: usize,
    ) -> Option<PopEditable> {
        let editable = self
            .items
            .iter()
            .filter(|item| item.command.is_editable())
            .cloned()
            .collect::<Vec<_>>();
        if editable.is_empty() {
            return None;
        }
        let texts = editable
            .iter()
            .filter_map(|item| item.command.text())
            .collect::<Vec<_>>();
        let prefix = texts.join("\n");
        let text = texts
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(current_input))
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        // Source JS cursor offsets are UTF-16 code units, not UTF-8 bytes.
        let cursor_offset = prefix.encode_utf16().count() + 1 + cursor_offset;
        let mut images = Vec::new();
        let mut next_image_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before Unix epoch")
            .as_millis() as u64;
        for item in &editable {
            if let Some(pastes) = item
                .command
                .pasted_contents
                .as_ref()
                .and_then(Value::as_object)
            {
                let mut entries = pastes.iter().collect::<Vec<_>>();
                // Object.values on integer-key JS maps enumerates keys numerically.
                entries.sort_by(
                    |(a, _), (b, _)| match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(a), Ok(b)) => a.cmp(&b),
                        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                        _ => a.cmp(b),
                    },
                );
                for (_, content) in entries {
                    if content.get("type").and_then(Value::as_str) == Some("image") {
                        images.push(content.clone());
                    }
                }
            }
            if let Some(blocks) = item.command.value.as_array() {
                let mut image_index = 0u64;
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) == Some("image")
                        && block.pointer("/source/type").and_then(Value::as_str) == Some("base64")
                    {
                        images.push(serde_json::json!({
                            "id":next_image_id+image_index,"type":"image",
                            "content":block.pointer("/source/data"),
                            "mediaType":block.pointer("/source/media_type"),
                            "filename":format!("image{}",image_index+1)
                        }));
                        image_index += 1;
                    }
                }
                next_image_id += image_index;
            }
        }
        self.items.retain(|item| !item.command.is_editable());
        self.notify();
        for item in &editable {
            self.record("popAll", item.command.value.as_str());
        }
        Some(PopEditable {
            text,
            cursor_offset,
            images,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn cmd(text: &str, priority: Option<Priority>, mode: Mode) -> Command {
        Command {
            value: json!(text),
            mode,
            priority,
            uuid: None,
            orphaned_permission: None,
            pasted_contents: None,
            pre_expansion_value: None,
            skip_slash_commands: false,
            bridge_origin: false,
            is_meta: false,
            origin: None,
            workload: None,
            agent_id: None,
        }
    }
    #[test]
    fn priority_fifo_and_nonmatching_agent_isolation() {
        let mut q = Queue::default();
        q.enqueue(
            cmd("later", Some(Priority::Later), Mode::TaskNotification),
            false,
        );
        let first = q.enqueue(cmd("next1", None, Mode::Prompt), false);
        let second = q.enqueue(cmd("next2", None, Mode::Prompt), false);
        q.enqueue(cmd("now", Some(Priority::Now), Mode::Bash), false);
        assert_eq!(
            q.peek(|_| true).unwrap().command.text().as_deref(),
            Some("now")
        );
        assert_eq!(q.dequeue(|_| true).unwrap().command.text().unwrap(), "now");
        assert_eq!(q.dequeue(|_| true).unwrap().identity, first);
        assert_eq!(q.dequeue(|_| true).unwrap().identity, second);
        assert_eq!(
            q.dequeue(|_| true).unwrap().command.text().unwrap(),
            "later"
        );
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }
    #[test]
    fn matching_remove_peek_and_raw_dequeue_all_order() {
        let mut q = Queue::default();
        for (text, priority) in [
            ("late", Priority::Later),
            ("now", Priority::Now),
            ("next", Priority::Next),
        ] {
            q.enqueue(cmd(text, Some(priority), Mode::Prompt), false);
        }
        assert_eq!(
            q.by_max_priority(Priority::Next)
                .iter()
                .map(|c| c.command.text().unwrap())
                .collect::<Vec<_>>(),
            vec!["now", "next"]
        );
        let removed = q.remove_matching(|c| c.priority() == Priority::Now);
        assert_eq!(removed[0].command.text().unwrap(), "now");
        assert_eq!(
            q.dequeue_all()
                .iter()
                .map(|c| c.command.text().unwrap())
                .collect::<Vec<_>>(),
            vec!["late", "next"]
        );
    }
    #[test]
    fn editable_pop_preserves_notifications_raw_xml_and_utf16_cursor() {
        let mut q = Queue::default();
        q.enqueue(cmd("hello 🌐", None, Mode::Prompt), false);
        q.enqueue(cmd("<notification/>", None, Mode::TaskNotification), true);
        let mut meta = cmd("<system/>", None, Mode::Prompt);
        meta.is_meta = true;
        q.enqueue(meta, false);
        let result = q.pop_all_editable("draft", 3).unwrap();
        assert_eq!(result.text, "hello 🌐\ndraft");
        assert_eq!(
            result.cursor_offset,
            "hello 🌐".encode_utf16().count() + 1 + 3
        );
        assert_eq!(q.len(), 2);
        assert!(!q.snapshot()[0].command.is_editable());
        assert!(!q.snapshot()[1].command.is_editable());
        assert!(q.pop_all_editable("", 0).is_none());
    }
    #[test]
    fn images_slashes_revision_and_identity() {
        let mut q = Queue::default();
        let mut c = cmd("/model local", None, Mode::Prompt);
        assert!(c.is_slash());
        c.skip_slash_commands = true;
        assert!(!c.is_slash());
        c.pasted_contents = Some(json!({"11":{"type":"image","id":11,"content":"YWJj"}}));
        let id = q.enqueue(c, false);
        assert!(q.snapshot()[0].command.contains_image());
        let result = q.pop_all_editable("", 0).unwrap();
        assert_eq!(result.images[0]["id"], 11);
        assert!(q.revision() > 1);
        assert_eq!(q.remove_identity(&[id]), 0);
        let rev = q.revision();
        q.clear();
        assert_eq!(q.revision(), rev);
    }
    #[test]
    fn subscribers_receive_synchronous_notifications_and_channels_remain_noneditable() {
        use std::{cell::Cell, rc::Rc};
        let mut queue = Queue::default();
        let count = Rc::new(Cell::new(0));
        let shared = count.clone();
        let sub = queue.subscribe(move || shared.set(shared.get() + 1));
        queue.enqueue(cmd("first", None, Mode::Prompt), false);
        queue.recheck();
        assert_eq!(count.get(), 2);
        queue.unsubscribe(sub);
        queue.clear();
        assert_eq!(count.get(), 2);
        let mut channel = cmd("<remote/>", None, Mode::Prompt);
        channel.is_meta = true;
        channel.origin = Some(json!({"kind":"channel"}));
        assert!(!channel.is_editable());
        assert!(!channel.is_visible(false));
        assert!(channel.is_visible(true));
    }
    #[test]
    fn image_paste_numeric_key_enumeration_and_remote_image_ids() {
        let mut queue = Queue::default();
        let mut c = cmd("prompt", None, Mode::Prompt);
        c.pasted_contents = Some(json!({
            "12":{"id":12,"type":"image","content":"a"},
            "2":{"id":2,"type":"image","content":"b"}
        }));
        c.value = json!([{"type":"text","text":"prompt"},
            {"type":"image","source":{"type":"base64","data":"AQID","media_type":"image/png"}}]);
        queue.enqueue(c, false);
        let edited = queue.pop_all_editable("", 0).unwrap();
        assert_eq!(edited.images[0]["id"], 2);
        assert_eq!(edited.images[1]["id"], 12);
        assert!(edited.images[2]["id"].as_u64().unwrap() > 1_700_000_000_000);
    }
    #[test]
    fn assigned_agents_preserve_filtered_priority_order() {
        let mut q = Queue::default();
        let mut sub = cmd("sub", Some(Priority::Now), Mode::TaskNotification);
        sub.agent_id = Some("worker".into());
        q.enqueue(sub, false);
        q.enqueue(cmd("main", Some(Priority::Next), Mode::Prompt), false);
        let next = q.dequeue(|c| c.agent_id.is_none()).unwrap();
        assert_eq!(next.command.text().unwrap(), "main");
        assert_eq!(q.len(), 1);
    }
}
