//! Native Rust UI structure derived from original: src/components/Spinner.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("width".into(), Value::String("100%".into())),
            ("alignItems".into(), Value::String("flex-start".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "SpinnerAnimationRow".into(),
                attrs: vec![
                    ("mode".into(), Value::Null),
                    ("reducedMotion".into(), Value::Null),
                    ("hasActiveTools".into(), Value::Null),
                    ("responseLengthRef".into(), Value::Null),
                    ("message".into(), Value::Null),
                    ("messageColor".into(), Value::Null),
                    ("shimmerColor".into(), Value::Null),
                    ("overrideColor".into(), Value::Null),
                    ("loadingStartTimeRef".into(), Value::Null),
                    ("totalPausedMsRef".into(), Value::Null),
                    ("pauseStartTimeRef".into(), Value::Null),
                    ("spinnerSuffix".into(), Value::Null),
                    ("verbose".into(), Value::Null),
                    ("columns".into(), Value::Null),
                    ("hasRunningTeammates".into(), Value::Null),
                    ("teammateTokens".into(), Value::Null),
                    ("foregroundedTeammate".into(), Value::Null),
                    ("leaderIsIdle".into(), Value::Null),
                    ("thinkingStatus".into(), Value::Null),
                    ("effortSuffix".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
            VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                Value::Null
            } else {
                if crate::generated_ports::runtime::truthy(&Value::Null) {
                    Value::Null
                } else {
                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::Null
                    } else {
                        Value::Null
                    }
                }
            }),
            VNode::Empty,
        ],
    }
}
