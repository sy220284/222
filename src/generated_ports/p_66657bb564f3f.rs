//! Rust exports derived from original: src/utils/ide.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IdeType {
    #[serde(rename = "cursor")]
    cursor,
    #[serde(rename = "windsurf")]
    windsurf,
    #[serde(rename = "vscode")]
    vscode,
    #[serde(rename = "pycharm")]
    pycharm,
    #[serde(rename = "intellij")]
    intellij,
    #[serde(rename = "webstorm")]
    webstorm,
    #[serde(rename = "phpstorm")]
    phpstorm,
    #[serde(rename = "rubymine")]
    rubymine,
    #[serde(rename = "clion")]
    clion,
    #[serde(rename = "goland")]
    goland,
    #[serde(rename = "rider")]
    rider,
    #[serde(rename = "datagrip")]
    datagrip,
    #[serde(rename = "appcode")]
    appcode,
    #[serde(rename = "dataspell")]
    dataspell,
    #[serde(rename = "aqua")]
    aqua,
    #[serde(rename = "gateway")]
    gateway,
    #[serde(rename = "fleet")]
    fleet,
    #[serde(rename = "androidstudio")]
    androidstudio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDEExtensionInstallationStatus {
    #[serde(rename = "installed")]
    pub installed: bool,
    #[serde(rename = "error")]
    pub error: Value,
    #[serde(rename = "installedVersion")]
    pub installedVersion: Value,
    #[serde(rename = "ideType")]
    pub ideType: Value,
}
