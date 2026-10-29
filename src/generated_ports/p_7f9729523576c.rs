//! Rust exports derived from original: src/ink/styles.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnsiColor {
    #[serde(rename = "ansi:black")]
    ansi_black,
    #[serde(rename = "ansi:red")]
    ansi_red,
    #[serde(rename = "ansi:green")]
    ansi_green,
    #[serde(rename = "ansi:yellow")]
    ansi_yellow,
    #[serde(rename = "ansi:blue")]
    ansi_blue,
    #[serde(rename = "ansi:magenta")]
    ansi_magenta,
    #[serde(rename = "ansi:cyan")]
    ansi_cyan,
    #[serde(rename = "ansi:white")]
    ansi_white,
    #[serde(rename = "ansi:blackBright")]
    ansi_blackBright,
    #[serde(rename = "ansi:redBright")]
    ansi_redBright,
    #[serde(rename = "ansi:greenBright")]
    ansi_greenBright,
    #[serde(rename = "ansi:yellowBright")]
    ansi_yellowBright,
    #[serde(rename = "ansi:blueBright")]
    ansi_blueBright,
    #[serde(rename = "ansi:magentaBright")]
    ansi_magentaBright,
    #[serde(rename = "ansi:cyanBright")]
    ansi_cyanBright,
    #[serde(rename = "ansi:whiteBright")]
    ansi_whiteBright,
}
