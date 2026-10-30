//! Native Rust port derived from original: src/utils/classifierApprovals.ts
//!
//! Tracks which tool uses were auto-approved by the bash / auto-mode
//! classifiers, keyed by toolUseID, read back by the tool-success
//! transcript renderer. The original gates every mutation behind
//! compile-time bundle features (`BASH_CLASSIFIER`,
//! `TRANSCRIPT_CLASSIFIER`); the Rust port uses process-lifetime switches
//! (defaults on, matching a production build). Signals map to
//! `event_signal::Signal`.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::event_signal::Signal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifierKind {
    Bash,
    AutoMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifierApproval {
    pub classifier: ClassifierKind,
    pub matched_rule: Option<String>,
    pub reason: Option<String>,
}

pub const FEATURE_BASH_CLASSIFIER: AtomicBool = AtomicBool::new(true);
pub const FEATURE_TRANSCRIPT_CLASSIFIER: AtomicBool = AtomicBool::new(true);

pub fn bash_classifier_enabled() -> bool {
    FEATURE_BASH_CLASSIFIER.load(Ordering::Relaxed)
}

pub fn transcript_classifier_enabled() -> bool {
    FEATURE_TRANSCRIPT_CLASSIFIER.load(Ordering::Relaxed)
}

struct ClassifierState {
    approvals: HashMap<String, ClassifierApproval>,
    checking: HashSet<String>,
    checking_signal: Signal<()>,
}

fn state_slot() -> &'static Mutex<ClassifierState> {
    static SLOT: OnceLock<Mutex<ClassifierState>> = OnceLock::new();
    SLOT.get_or_init(|| {
        Mutex::new(ClassifierState {
            approvals: HashMap::new(),
            checking: HashSet::new(),
            checking_signal: Signal::new(),
        })
    })
}

/// Records a bash-classifier auto-approval for a tool use.
pub fn set_classifier_approval(tool_use_id: &str, matched_rule: &str) {
    if !bash_classifier_enabled() {
        return;
    }
    let mut state = state_slot().lock().unwrap();
    state.approvals.insert(
        tool_use_id.to_string(),
        ClassifierApproval {
            classifier: ClassifierKind::Bash,
            matched_rule: Some(matched_rule.to_string()),
            reason: None,
        },
    );
}

/// Returns the matched bash-classifier rule for a tool use, if any.
pub fn get_classifier_approval(tool_use_id: &str) -> Option<String> {
    if !bash_classifier_enabled() {
        return None;
    }
    let state = state_slot().lock().unwrap();
    state
        .approvals
        .get(tool_use_id)
        .filter(|a| a.classifier == ClassifierKind::Bash)
        .and_then(|a| a.matched_rule.clone())
}

/// Records an auto-mode ("yolo") classifier approval with a reason.
pub fn set_yolo_classifier_approval(tool_use_id: &str, reason: &str) {
    if !transcript_classifier_enabled() {
        return;
    }
    let mut state = state_slot().lock().unwrap();
    state.approvals.insert(
        tool_use_id.to_string(),
        ClassifierApproval {
            classifier: ClassifierKind::AutoMode,
            matched_rule: None,
            reason: Some(reason.to_string()),
        },
    );
}

/// Returns the auto-mode classifier reason for a tool use, if any.
pub fn get_yolo_classifier_approval(tool_use_id: &str) -> Option<String> {
    if !transcript_classifier_enabled() {
        return None;
    }
    let state = state_slot().lock().unwrap();
    state
        .approvals
        .get(tool_use_id)
        .filter(|a| a.classifier == ClassifierKind::AutoMode)
        .and_then(|a| a.reason.clone())
}

/// Marks a tool use as currently under classifier checking.
pub fn set_classifier_checking(tool_use_id: &str) {
    if !bash_classifier_enabled() && !transcript_classifier_enabled() {
        return;
    }
    let mut state = state_slot().lock().unwrap();
    state.checking.insert(tool_use_id.to_string());
    state.checking_signal.emit(());
}

/// Clears the in-checking marker for a tool use.
pub fn clear_classifier_checking(tool_use_id: &str) {
    if !bash_classifier_enabled() && !transcript_classifier_enabled() {
        return;
    }
    let mut state = state_slot().lock().unwrap();
    state.checking.remove(tool_use_id);
    state.checking_signal.emit(());
}

/// Whether a tool use is currently under classifier checking.
pub fn is_classifier_checking(tool_use_id: &str) -> bool {
    let state = state_slot().lock().unwrap();
    state.checking.contains(tool_use_id)
}

/// Drops a single approval record (no feature gate in the original).
pub fn delete_classifier_approval(tool_use_id: &str) {
    let mut state = state_slot().lock().unwrap();
    state.approvals.remove(tool_use_id);
}

/// Clears every approval record and checking marker.
pub fn clear_classifier_approvals() {
    let mut state = state_slot().lock().unwrap();
    state.approvals.clear();
    state.checking.clear();
    state.checking_signal.emit(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_approval_roundtrip_and_kind_isolation() {
        clear_classifier_approvals();
        set_classifier_approval("t1", "rule: allow ls");
        assert_eq!(
            get_classifier_approval("t1").as_deref(),
            Some("rule: allow ls")
        );
        // Auto-mode query must not see bash approval.
        assert_eq!(get_yolo_classifier_approval("t1"), None);
        delete_classifier_approval("t1");
        assert_eq!(get_classifier_approval("t1"), None);
    }

    #[test]
    fn yolo_approval_roundtrip_and_kind_isolation() {
        clear_classifier_approvals();
        set_yolo_classifier_approval("t2", "transcript matched safe read");
        assert_eq!(
            get_yolo_classifier_approval("t2").as_deref(),
            Some("transcript matched safe read")
        );
        assert_eq!(get_classifier_approval("t2"), None);
        clear_classifier_approvals();
        assert_eq!(get_yolo_classifier_approval("t2"), None);
    }

    #[test]
    fn checking_lifecycle_and_clear_all() {
        clear_classifier_approvals();
        assert!(!is_classifier_checking("t3"));
        set_classifier_checking("t3");
        assert!(is_classifier_checking("t3"));
        clear_classifier_checking("t3");
        assert!(!is_classifier_checking("t3"));
        set_classifier_checking("t4");
        clear_classifier_approvals();
        assert!(!is_classifier_checking("t4"));
    }
}
