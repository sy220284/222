//! Rust exports derived from original: src/components/FeedbackSurvey/submitTranscriptShare.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TranscriptShareTrigger {
    #[serde(rename = "bad_feedback_survey")]
    bad_feedback_survey,
    #[serde(rename = "good_feedback_survey")]
    good_feedback_survey,
    #[serde(rename = "frustration")]
    frustration,
    #[serde(rename = "memory_survey")]
    memory_survey,
}
