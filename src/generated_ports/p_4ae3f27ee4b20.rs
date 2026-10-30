//! Hand port of `src/components/PromptInput/PromptInputModeIndicator.tsx`.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json,Value};
use std::collections::BTreeMap;

fn theme_color(name:&str)->Option<String> {
    if ["red","blue","green","yellow","purple","orange","pink","cyan"].contains(&name) {
        Some(format!("{name}_FOR_SUBAGENTS_ONLY"))
    } else {None}
}
pub fn render_source_view(props:&Value)->VNode {
    let is_loading=props.get("isLoading").and_then(Value::as_bool).unwrap_or(false);
    let viewing=props.get("viewingAgentName").and_then(Value::as_str).is_some_and(|s|!s.is_empty());
    let bash=!viewing && props.get("mode").and_then(Value::as_str)==Some("bash");
    let team_color=if viewing {
        props.get("viewingAgentColor").and_then(Value::as_str).and_then(theme_color)
    } else if props.get("agentSwarmsEnabled").and_then(Value::as_bool)==Some(true) {
        props.get("teammateColor").and_then(Value::as_str).and_then(theme_color)
    } else {None};
    let mut attrs=BTreeMap::from([("dimColor".into(),json!(is_loading))]);
    if bash {attrs.insert("color".into(),json!("bashBorder"));}
    else if let Some(color)=team_color {attrs.insert("color".into(),json!(color));}
    let symbol=if bash {"!\u{a0}"} else {"❯\u{a0}"};
    VNode::Element{tag:"Box".into(),attrs:BTreeMap::from([
        ("alignItems".into(),json!("flex-start")),("alignSelf".into(),json!("flex-start")),
        ("flexWrap".into(),json!("nowrap")),("justifyContent".into(),json!("flex-start"))]),
        children:vec![VNode::Element{tag:"Text".into(),attrs,children:vec![VNode::Text(symbol.into())]}]}
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn input_modes_follow_the_source_precedence() {
        let p=json!({"mode":"bash","isLoading":true});
        let x=render_source_view(&p);assert_eq!(x.text_content(),"!\u{a0}");
        let VNode::Element{children,..}=x else {panic!("box")};
        let VNode::Element{attrs,..}=&children[0] else {panic!("text")};
        assert_eq!(attrs["dimColor"],true); assert_eq!(attrs["color"],"bashBorder");
        let p=json!({"mode":"bash","viewingAgentName":"worker","viewingAgentColor":"blue"});
        let x=render_source_view(&p);assert_eq!(x.text_content(),"❯\u{a0}");
        let VNode::Element{children,..}=x else {panic!("box")};
        let VNode::Element{attrs,..}=&children[0] else {panic!("text")};
        assert_eq!(attrs["color"],"blue_FOR_SUBAGENTS_ONLY");
    }
}
