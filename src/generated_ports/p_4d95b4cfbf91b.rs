//! Rust exports derived from original: src/utils/attachments.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn TODO_REMINDER_CONFIG()->Value{serde_json::from_str(r#"{"TURNS_SINCE_WRITE":10,"TURNS_BETWEEN_REMINDERS":10}"#).expect("Valid source constant")}

pub fn PLAN_MODE_ATTACHMENT_CONFIG()->Value{serde_json::from_str(r#"{"TURNS_BETWEEN_ATTACHMENTS":5,"FULL_REMINDER_EVERY_N_ATTACHMENTS":5}"#).expect("Valid source constant")}

pub fn AUTO_MODE_ATTACHMENT_CONFIG()->Value{serde_json::from_str(r#"{"TURNS_BETWEEN_ATTACHMENTS":5,"FULL_REMINDER_EVERY_N_ATTACHMENTS":5}"#).expect("Valid source constant")}

pub fn VERIFY_PLAN_REMINDER_CONFIG()->Value{serde_json::from_str(r#"{"TURNS_BETWEEN_REMINDERS":10}"#).expect("Valid source constant")}
