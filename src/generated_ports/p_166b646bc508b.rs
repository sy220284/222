//! Rust exports derived from original: src/native-ts/yoga-layout/enums.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn Align() -> Value {
    serde_json::from_str(r#"{"Auto":0,"FlexStart":1,"Center":2,"FlexEnd":3,"Stretch":4,"Baseline":5,"SpaceBetween":6,"SpaceAround":7,"SpaceEvenly":8}"#).expect("Valid source constant")
}

pub fn BoxSizing() -> Value {
    serde_json::from_str(r#"{"BorderBox":0,"ContentBox":1}"#).expect("Valid source constant")
}

pub fn Dimension() -> Value {
    serde_json::from_str(r#"{"Width":0,"Height":1}"#).expect("Valid source constant")
}

pub fn Direction() -> Value {
    serde_json::from_str(r#"{"Inherit":0,"LTR":1,"RTL":2}"#).expect("Valid source constant")
}

pub fn Display() -> Value {
    serde_json::from_str(r#"{"Flex":0,"None":1,"Contents":2}"#).expect("Valid source constant")
}

pub fn Edge() -> Value {
    serde_json::from_str(r#"{"Left":0,"Top":1,"Right":2,"Bottom":3,"Start":4,"End":5,"Horizontal":6,"Vertical":7,"All":8}"#).expect("Valid source constant")
}

pub fn Errata() -> Value {
    serde_json::from_str(r#"{"None":0,"StretchFlexBasis":1,"AbsolutePositionWithoutInsetsExcludesPadding":2,"AbsolutePercentAgainstInnerSize":4,"All":2147483647,"Classic":2147483646}"#).expect("Valid source constant")
}

pub fn ExperimentalFeature() -> Value {
    serde_json::from_str(r#"{"WebFlexBasis":0}"#).expect("Valid source constant")
}

pub fn FlexDirection() -> Value {
    serde_json::from_str(r#"{"Column":0,"ColumnReverse":1,"Row":2,"RowReverse":3}"#)
        .expect("Valid source constant")
}

pub fn Gutter() -> Value {
    serde_json::from_str(r#"{"Column":0,"Row":1,"All":2}"#).expect("Valid source constant")
}

pub fn Justify() -> Value {
    serde_json::from_str(r#"{"FlexStart":0,"Center":1,"FlexEnd":2,"SpaceBetween":3,"SpaceAround":4,"SpaceEvenly":5}"#).expect("Valid source constant")
}

pub fn MeasureMode() -> Value {
    serde_json::from_str(r#"{"Undefined":0,"Exactly":1,"AtMost":2}"#)
        .expect("Valid source constant")
}

pub fn Overflow() -> Value {
    serde_json::from_str(r#"{"Visible":0,"Hidden":1,"Scroll":2}"#).expect("Valid source constant")
}

pub fn PositionType() -> Value {
    serde_json::from_str(r#"{"Static":0,"Relative":1,"Absolute":2}"#)
        .expect("Valid source constant")
}

pub fn Unit() -> Value {
    serde_json::from_str(r#"{"Undefined":0,"Point":1,"Percent":2,"Auto":3}"#)
        .expect("Valid source constant")
}

pub fn Wrap() -> Value {
    serde_json::from_str(r#"{"NoWrap":0,"Wrap":1,"WrapReverse":2}"#).expect("Valid source constant")
}
