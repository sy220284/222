//! Rust exports derived from original: src/utils/messages.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn INTERRUPT_MESSAGE()->Value{serde_json::from_str(r#""[Request interrupted by user]""#).expect("Valid source constant")}

pub fn INTERRUPT_MESSAGE_FOR_TOOL_USE()->Value{serde_json::from_str(r#""[Request interrupted by user for tool use]""#).expect("Valid source constant")}

pub fn CANCEL_MESSAGE()->Value{serde_json::from_str(r#""The user doesn't want to take this action right now. STOP what you are doing and wait for the user to tell you how to proceed.""#).expect("Valid source constant")}

pub fn REJECT_MESSAGE()->Value{serde_json::from_str(r#""The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.""#).expect("Valid source constant")}

pub fn REJECT_MESSAGE_WITH_REASON_PREFIX()->Value{serde_json::from_str(r#""The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). To tell you how to proceed, the user said:\n""#).expect("Valid source constant")}

pub fn SUBAGENT_REJECT_MESSAGE()->Value{serde_json::from_str(r#""Permission for this tool use was denied. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). Try a different approach or report the limitation to complete your task.""#).expect("Valid source constant")}

pub fn SUBAGENT_REJECT_MESSAGE_WITH_REASON_PREFIX()->Value{serde_json::from_str(r#""Permission for this tool use was denied. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). The user said:\n""#).expect("Valid source constant")}

pub fn PLAN_REJECTION_PREFIX()->Value{serde_json::from_str(r#""The agent proposed a plan that was rejected by the user. The user chose to stay in plan mode rather than proceed with implementation.\n\nRejected plan:\n""#).expect("Valid source constant")}

pub fn NO_RESPONSE_REQUESTED()->Value{serde_json::from_str(r#""No response requested.""#).expect("Valid source constant")}

pub fn SYNTHETIC_TOOL_RESULT_PLACEHOLDER()->Value{serde_json::from_str(r#""[Tool result missing due to internal error]""#).expect("Valid source constant")}

pub fn SYNTHETIC_MODEL()->Value{serde_json::from_str(r#""<synthetic>""#).expect("Valid source constant")}

pub fn PLAN_PHASE4_CONTROL()->Value{serde_json::from_str(r####""### Phase 4: Final Plan\nGoal: Write your final plan to the plan file (the only file you can edit).\n- Begin with a **Context** section: explain why this change is being made — the problem or need it addresses, what prompted it, and the intended outcome\n- Include only your recommended approach, not all alternatives\n- Ensure that the plan file is concise enough to scan quickly, but detailed enough to execute effectively\n- Include the paths of critical files to be modified\n- Reference existing functions and utilities you found that should be reused, with their file paths\n- Include a verification section describing how to test the changes end-to-end (run the code, use MCP tools, run tests)""####).expect("Valid source constant")}

use serde_json::json;

pub fn buildClassifierUnavailableMessage(args:&[Value])->Value {super::runtime::add(&super::runtime::add(&super::runtime::add(&super::runtime::concat(&[Value::String("".into()),args.get(1).cloned().unwrap_or(Value::Null),Value::String(" is temporarily unavailable, so auto mode cannot determine the safety of ".into()),args.get(0).cloned().unwrap_or(Value::Null),Value::String(" right now. ".into())]),&Value::String("Wait briefly and then try this action again. ".into())),&Value::String("If it keeps failing, continue with other tasks that don't require this action and come back to it later. ".into())),&Value::String("Note: reading files, searching code, and other read-only operations do not require the classifier and can still be used.".into()))}
pub fn isSystemLocalCommandMessage(args:&[Value])->Value {super::runtime::and(&super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"type"),&Value::String("system".into())),&super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"subtype"),&Value::String("local_command".into())))}
pub fn wrapInSystemReminder(args:&[Value])->Value {super::runtime::concat(&[Value::String("<system-reminder>\n".into()),args.get(0).cloned().unwrap_or(Value::Null),Value::String("\n</system-reminder>".into())])}
pub fn isCompactBoundaryMessage(args:&[Value])->Value {super::runtime::and(&super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"type"),&Value::String("system".into())),&super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"subtype"),&Value::String("compact_boundary".into())))}
