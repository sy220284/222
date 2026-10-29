//! Rust exports derived from original: src/types/permissions.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn EXTERNAL_PERMISSION_MODES()->Value{serde_json::from_str(r#"["acceptEdits","bypassPermissions","default","dontAsk","plan"]"#).expect("Valid source constant")}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PermissionBehavior{
    #[serde(rename="allow")]
    allow,
    #[serde(rename="deny")]
    deny,
    #[serde(rename="ask")]
    ask,
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PermissionRuleSource{
    #[serde(rename="userSettings")]
    userSettings,
    #[serde(rename="projectSettings")]
    projectSettings,
    #[serde(rename="localSettings")]
    localSettings,
    #[serde(rename="flagSettings")]
    flagSettings,
    #[serde(rename="policySettings")]
    policySettings,
    #[serde(rename="cliArg")]
    cliArg,
    #[serde(rename="command")]
    command,
    #[serde(rename="session")]
    session,
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PermissionUpdateDestination{
    #[serde(rename="userSettings")]
    userSettings,
    #[serde(rename="projectSettings")]
    projectSettings,
    #[serde(rename="localSettings")]
    localSettings,
    #[serde(rename="session")]
    session,
    #[serde(rename="cliArg")]
    cliArg,
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum ClassifierBehavior{
    #[serde(rename="deny")]
    deny,
    #[serde(rename="ask")]
    ask,
    #[serde(rename="allow")]
    allow,
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum RiskLevel{
    #[serde(rename="LOW")]
    LOW,
    #[serde(rename="MEDIUM")]
    MEDIUM,
    #[serde(rename="HIGH")]
    HIGH,
}
