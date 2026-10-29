//! Rust exports derived from original: src/utils/nativeInstaller/packageManagers.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PackageManager{
    #[serde(rename="homebrew")]
    homebrew,
    #[serde(rename="winget")]
    winget,
    #[serde(rename="pacman")]
    pacman,
    #[serde(rename="deb")]
    deb,
    #[serde(rename="rpm")]
    rpm,
    #[serde(rename="apk")]
    apk,
    #[serde(rename="mise")]
    mise,
    #[serde(rename="asdf")]
    asdf,
    #[serde(rename="unknown")]
    unknown,
}
