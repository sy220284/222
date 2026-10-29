//! Rust exports derived from original: src/buddy/types.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn RARITIES()->Value{serde_json::from_str(r#"["common","uncommon","rare","epic","legendary"]"#).expect("Valid source constant")}

pub fn EYES()->Value{serde_json::from_str(r#"["·","✦","×","◉","@","°"]"#).expect("Valid source constant")}

pub fn HATS()->Value{serde_json::from_str(r#"["none","crown","tophat","propeller","halo","wizard","beanie","tinyduck"]"#).expect("Valid source constant")}

pub fn STAT_NAMES()->Value{serde_json::from_str(r#"["DEBUGGING","PATIENCE","CHAOS","WISDOM","SNARK"]"#).expect("Valid source constant")}

pub fn RARITY_WEIGHTS()->Value{serde_json::from_str(r#"{"common":60,"uncommon":25,"rare":10,"epic":4,"legendary":1}"#).expect("Valid source constant")}

pub fn RARITY_STARS()->Value{serde_json::from_str(r#"{"common":"★","uncommon":"★★","rare":"★★★","epic":"★★★★","legendary":"★★★★★"}"#).expect("Valid source constant")}

pub fn RARITY_COLORS()->Value{serde_json::from_str(r#"{"common":"inactive","uncommon":"success","rare":"permission","epic":"autoAccept","legendary":"warning"}"#).expect("Valid source constant")}
