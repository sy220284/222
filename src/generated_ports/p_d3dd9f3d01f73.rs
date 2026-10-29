//! Rust exports derived from original: src/utils/autoRunIssue.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum AutoRunIssueReason{
    #[serde(rename="feedback_survey_bad")]
    feedback_survey_bad,
    #[serde(rename="feedback_survey_good")]
    feedback_survey_good,
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into())),("marginTop".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Value(Value::Null),VNode::Element{tag:"Box".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("dimColor".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Text("Reason: ".into()),VNode::Value(Value::Null)]}]}]}
}
