//! Manual port of `src/components/PromptInput/VoiceIndicator.tsx`.
//! The animation clock, voice state, and accessibility setting are provided
//! by the host. This view performs the same 2s RGB interpolation as source.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn node(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}
fn text(body: &str, attrs: BTreeMap<String, Value>) -> VNode {
    node("Text", attrs, vec![VNode::Text(body.into())])
}
pub fn voice_warmup_hint(props: &Value) -> VNode {
    if props.get("voiceFeatureEnabled").and_then(Value::as_bool) != Some(true) {
        return VNode::Empty;
    }
    text(
        "keep holding…",
        BTreeMap::from([("dimColor".into(), json!(true))]),
    )
}
pub fn render_source_view(props: &Value) -> VNode {
    if props.get("voiceFeatureEnabled").and_then(Value::as_bool) != Some(true) {
        return VNode::Empty;
    }
    match props
        .get("voiceState")
        .and_then(Value::as_str)
        .unwrap_or("idle")
    {
        "recording" => text(
            "listening…",
            BTreeMap::from([("dimColor".into(), json!(true))]),
        ),
        "processing" => {
            if props.get("prefersReducedMotion").and_then(Value::as_bool) == Some(true) {
                return text(
                    "Voice: processing…",
                    BTreeMap::from([("color".into(), json!("warning"))]),
                );
            }
            let ms = props
                .get("animationTimeMs")
                .and_then(Value::as_f64)
                .unwrap_or(0.)
                .max(0.);
            let elapsed = ms / 1000.;
            let opacity = ((elapsed * std::f64::consts::PI * 2. / 2.).sin() + 1.) / 2.;
            let component = (153. + (185. - 153.) * opacity).round() as u8;
            let color = format!("rgb({component},{component},{component})");
            node(
                "Box",
                BTreeMap::new(),
                vec![text(
                    "Voice: processing…",
                    BTreeMap::from([("color".into(), json!(color))]),
                )],
            )
        }
        _ => VNode::Empty,
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn feature_off_and_idle_do_not_render() {
        assert_eq!(
            render_source_view(&json!({"voiceFeatureEnabled":false,"voiceState":"processing"})),
            VNode::Empty
        );
        assert_eq!(
            render_source_view(&json!({"voiceFeatureEnabled":true,"voiceState":"idle"})),
            VNode::Empty
        );
        assert_eq!(
            voice_warmup_hint(&json!({"voiceFeatureEnabled":false})),
            VNode::Empty
        );
    }
    #[test]
    fn recording_processing_and_reduced_motion_follow_source() {
        let recording =
            render_source_view(&json!({"voiceFeatureEnabled":true,"voiceState":"recording"}));
        assert_eq!(recording.text_content(), "listening…");
        let reduced = render_source_view(
            &json!({"voiceFeatureEnabled":true,"voiceState":"processing","prefersReducedMotion":true}),
        );
        let VNode::Element { attrs, .. } = reduced else {
            panic!("Text")
        };
        assert_eq!(attrs["color"], "warning");
        let animated = render_source_view(
            &json!({"voiceFeatureEnabled":true,"voiceState":"processing","animationTimeMs":500}),
        );
        let VNode::Element { children, .. } = animated else {
            panic!("Box")
        };
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("Text")
        };
        assert_eq!(attrs["color"], "rgb(185,185,185)");
        let start = render_source_view(
            &json!({"voiceFeatureEnabled":true,"voiceState":"processing","animationTimeMs":0}),
        );
        let VNode::Element { children, .. } = start else {
            panic!("Box")
        };
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("Text")
        };
        assert_eq!(attrs["color"], "rgb(169,169,169)");
        assert_eq!(
            voice_warmup_hint(&json!({"voiceFeatureEnabled":true})).text_content(),
            "keep holding…"
        );
    }
}
