//! Rust hand-port of the global kill/yank ring in `src/utils/Cursor.ts`.
//! A single synchronized ring is shared among terminal editors, matching the
//! source's process-wide module state without allowing data races.
use std::sync::{Mutex, OnceLock};

const MAX_KILLS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YankPop {
    pub text: String,
    pub start: usize,
    pub length: usize,
}

#[derive(Debug, Default)]
pub struct KillRing {
    kills: Vec<String>,
    index: usize,
    last_action_was_kill: bool,
    last_yank: Option<(usize, usize)>,
}

impl KillRing {
    pub fn push(&mut self, text: &str, prepend: bool) {
        if text.is_empty() {
            return;
        }
        if self.last_action_was_kill && !self.kills.is_empty() {
            if prepend {
                self.kills[0].insert_str(0, text);
            } else {
                self.kills[0].push_str(text);
            }
        } else {
            self.kills.insert(0, text.to_owned());
            self.kills.truncate(MAX_KILLS);
        }
        self.last_action_was_kill = true;
        self.last_yank = None;
    }
    pub fn last(&self) -> &str {
        self.kills.first().map(String::as_str).unwrap_or("")
    }
    pub fn get(&self, index: isize) -> &str {
        if self.kills.is_empty() {
            return "";
        }
        let normalized = index.rem_euclid(self.kills.len() as isize) as usize;
        &self.kills[normalized]
    }
    pub fn len(&self) -> usize {
        self.kills.len()
    }
    pub fn is_empty(&self) -> bool {
        self.kills.is_empty()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn reset_kill_accumulation(&mut self) {
        self.last_action_was_kill = false;
    }
    pub fn record_yank(&mut self, start: usize, length: usize) {
        self.last_yank = Some((start, length));
        self.index = 0;
    }
    pub fn can_yank_pop(&self) -> bool {
        self.last_yank.is_some() && self.kills.len() > 1
    }
    pub fn yank_pop(&mut self) -> Option<YankPop> {
        let (start, length) = self.last_yank?;
        if self.kills.len() <= 1 {
            return None;
        }
        self.index = (self.index + 1) % self.kills.len();
        Some(YankPop {
            text: self.kills[self.index].clone(),
            start,
            length,
        })
    }
    pub fn update_yank_length(&mut self, length: usize) {
        if let Some((start, _)) = self.last_yank {
            self.last_yank = Some((start, length));
        }
    }
    pub fn reset_yank_state(&mut self) {
        self.last_yank = None;
    }
}

static KILL_RING: OnceLock<Mutex<KillRing>> = OnceLock::new();
pub fn with_global_kill_ring<T>(f: impl FnOnce(&mut KillRing) -> T) -> T {
    let mutex = KILL_RING.get_or_init(|| Mutex::new(KillRing::default()));
    let mut guard = mutex.lock().expect("terminal kill ring state poisoned");
    f(&mut guard)
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn kill_commands_accumulate_in_source_order_and_reset_between_commands() {
        let mut ring = KillRing::default();
        ring.push("two", true);
        ring.push("one ", true);
        ring.push(" three", false);
        assert_eq!(ring.last(), "one two three");
        ring.reset_kill_accumulation();
        ring.push("new", false);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.get(-1), "one two three");
    }
    #[test]
    fn yank_pop_rotates_and_keeps_replacement_extent() {
        let mut ring = KillRing::default();
        for text in ["a", "b", "c"] {
            ring.push(text, false);
            ring.reset_kill_accumulation();
        }
        ring.record_yank(4, 1);
        assert_eq!(
            ring.yank_pop(),
            Some(YankPop {
                text: "b".into(),
                start: 4,
                length: 1
            })
        );
        ring.update_yank_length(2);
        assert_eq!(
            ring.yank_pop(),
            Some(YankPop {
                text: "a".into(),
                start: 4,
                length: 2
            })
        );
        assert_eq!(ring.yank_pop().unwrap().text, "c");
        ring.reset_yank_state();
        assert!(!ring.can_yank_pop());
    }
    #[test]
    fn blank_kills_do_not_mutate_ring_and_ring_is_capped() {
        let mut ring = KillRing::default();
        ring.push("", false);
        assert!(ring.is_empty());
        for i in 0..15 {
            ring.push(&i.to_string(), false);
            ring.reset_kill_accumulation();
        }
        assert_eq!(ring.len(), 10);
        assert_eq!(ring.last(), "14");
        assert_eq!(ring.get(9), "5");
        ring.clear();
        assert!(ring.is_empty());
    }
}
