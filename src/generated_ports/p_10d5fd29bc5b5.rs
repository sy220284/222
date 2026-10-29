//! Rust exports derived from original: src/utils/shell/specPrefix.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn DEPTH_RULES() -> Value {
    serde_json::from_str(r#"{"rg":2,"pre-commit":2,"gcloud":4,"gcloud compute":6,"gcloud beta":6,"aws":4,"az":4,"kubectl":3,"docker":3,"dotnet":3,"git push":2}"#).expect("Valid source constant")
}
