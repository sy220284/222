//! Rust exports derived from original: src/utils/ultraplan/ccrSession.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PollFailReason{
    #[serde(rename="terminated")]
    terminated,
    #[serde(rename="timeout_pending")]
    timeout_pending,
    #[serde(rename="timeout_no_plan")]
    timeout_no_plan,
    #[serde(rename="extract_marker_missing")]
    extract_marker_missing,
    #[serde(rename="network_or_unknown")]
    network_or_unknown,
    #[serde(rename="stopped")]
    stopped,
}

pub fn ULTRAPLAN_TELEPORT_SENTINEL()->Value{serde_json::from_str(r#""__ULTRAPLAN_TELEPORT_LOCAL__""#).expect("Valid source constant")}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum UltraplanPhase{
    #[serde(rename="running")]
    running,
    #[serde(rename="needs_input")]
    needs_input,
    #[serde(rename="plan_ready")]
    plan_ready,
}
