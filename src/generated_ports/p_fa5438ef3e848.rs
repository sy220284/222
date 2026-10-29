//! Rust exports derived from original: src/constants/betas.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn INTERLEAVED_THINKING_BETA_HEADER()->Value{serde_json::from_str(r#""interleaved-thinking-2025-05-14""#).expect("Valid source constant")}

pub fn CONTEXT_1M_BETA_HEADER()->Value{serde_json::from_str(r#""context-1m-2025-08-07""#).expect("Valid source constant")}

pub fn CONTEXT_MANAGEMENT_BETA_HEADER()->Value{serde_json::from_str(r#""context-management-2025-06-27""#).expect("Valid source constant")}

pub fn STRUCTURED_OUTPUTS_BETA_HEADER()->Value{serde_json::from_str(r#""structured-outputs-2025-12-15""#).expect("Valid source constant")}

pub fn WEB_SEARCH_BETA_HEADER()->Value{serde_json::from_str(r#""web-search-2025-03-05""#).expect("Valid source constant")}

pub fn TOOL_SEARCH_BETA_HEADER_1P()->Value{serde_json::from_str(r#""advanced-tool-use-2025-11-20""#).expect("Valid source constant")}

pub fn TOOL_SEARCH_BETA_HEADER_3P()->Value{serde_json::from_str(r#""tool-search-tool-2025-10-19""#).expect("Valid source constant")}

pub fn EFFORT_BETA_HEADER()->Value{serde_json::from_str(r#""effort-2025-11-24""#).expect("Valid source constant")}

pub fn TASK_BUDGETS_BETA_HEADER()->Value{serde_json::from_str(r#""task-budgets-2026-03-13""#).expect("Valid source constant")}

pub fn PROMPT_CACHING_SCOPE_BETA_HEADER()->Value{serde_json::from_str(r#""prompt-caching-scope-2026-01-05""#).expect("Valid source constant")}

pub fn FAST_MODE_BETA_HEADER()->Value{serde_json::from_str(r#""fast-mode-2026-02-01""#).expect("Valid source constant")}

pub fn REDACT_THINKING_BETA_HEADER()->Value{serde_json::from_str(r#""redact-thinking-2026-02-12""#).expect("Valid source constant")}

pub fn TOKEN_EFFICIENT_TOOLS_BETA_HEADER()->Value{serde_json::from_str(r#""token-efficient-tools-2026-03-28""#).expect("Valid source constant")}

pub fn ADVISOR_BETA_HEADER()->Value{serde_json::from_str(r#""advisor-tool-2026-03-01""#).expect("Valid source constant")}
