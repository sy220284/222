//! Rust exports derived from original: src/utils/shell/readOnlyCommandValidation.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum FlagArgType{
    #[serde(rename="none")]
    none,
    #[serde(rename="number")]
    number,
    #[serde(rename="string")]
    string,
    #[serde(rename="char")]
    char,
    #[serde(rename="{}")]
    __,
    #[serde(rename="EOF")]
    EOF,
}

pub fn DOCKER_READ_ONLY_COMMANDS()->Value{serde_json::from_str(r#"{"docker logs":{"safeFlags":{"--follow":"none","-f":"none","--tail":"string","-n":"string","--timestamps":"none","-t":"none","--since":"string","--until":"string","--details":"none"}},"docker inspect":{"safeFlags":{"--format":"string","-f":"string","--type":"string","--size":"none","-s":"none"}}}"#).expect("Valid source constant")}

pub fn RIPGREP_READ_ONLY_COMMANDS()->Value{serde_json::from_str(r#"{"rg":{"safeFlags":{"-e":"string","--regexp":"string","-f":"string","-i":"none","--ignore-case":"none","-S":"none","--smart-case":"none","-F":"none","--fixed-strings":"none","-w":"none","--word-regexp":"none","-v":"none","--invert-match":"none","-c":"none","--count":"none","-l":"none","--files-with-matches":"none","--files-without-match":"none","-n":"none","--line-number":"none","-o":"none","--only-matching":"none","-A":"number","--after-context":"number","-B":"number","--before-context":"number","-C":"number","--context":"number","-H":"none","-h":"none","--heading":"none","--no-heading":"none","-q":"none","--quiet":"none","--column":"none","-g":"string","--glob":"string","-t":"string","--type":"string","-T":"string","--type-not":"string","--type-list":"none","--hidden":"none","--no-ignore":"none","-u":"none","-m":"number","--max-count":"number","-d":"number","--max-depth":"number","-a":"none","--text":"none","-z":"none","-L":"none","--follow":"none","--color":"string","--json":"none","--stats":"none","--help":"none","--version":"none","--debug":"none","--":"none"}}}"#).expect("Valid source constant")}

pub fn EXTERNAL_READONLY_COMMANDS()->Value{serde_json::from_str(r#"["docker ps","docker images"]"#).expect("Valid source constant")}
