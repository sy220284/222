//! Rust exports derived from original: src/ink/termio/ansi.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn C0()->Value{serde_json::from_str(r#"{"NUL":0,"SOH":1,"STX":2,"ETX":3,"EOT":4,"ENQ":5,"ACK":6,"BEL":7,"BS":8,"HT":9,"LF":10,"VT":11,"FF":12,"CR":13,"SO":14,"SI":15,"DLE":16,"DC1":17,"DC2":18,"DC3":19,"DC4":20,"NAK":21,"SYN":22,"ETB":23,"CAN":24,"EM":25,"SUB":26,"ESC":27,"FS":28,"GS":29,"RS":30,"US":31,"DEL":127}"#).expect("Valid source constant")}

pub fn ESC()->Value{serde_json::from_str(r#""\u001b""#).expect("Valid source constant")}

pub fn BEL()->Value{serde_json::from_str(r#""\u0007""#).expect("Valid source constant")}

pub fn SEP()->Value{serde_json::from_str(r#"";""#).expect("Valid source constant")}

pub fn ESC_TYPE()->Value{serde_json::from_str(r#"{"CSI":91,"OSC":93,"DCS":80,"APC":95,"PM":94,"SOS":88,"ST":92}"#).expect("Valid source constant")}

use serde_json::json;

pub fn isC0(args:&[Value])->Value {super::runtime::or(&super::runtime::lt(&args.get(0).cloned().unwrap_or(Value::Null),&json!(32)),&super::runtime::strict_equal(&args.get(0).cloned().unwrap_or(Value::Null),&json!(127)))}
pub fn isEscFinal(args:&[Value])->Value {super::runtime::and(&super::runtime::gte(&args.get(0).cloned().unwrap_or(Value::Null),&json!(48)),&super::runtime::lte(&args.get(0).cloned().unwrap_or(Value::Null),&json!(126)))}
