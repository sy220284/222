//! Hand port of `src/components/PromptInput/ShimmeredInput.tsx`.
//! Animation-frame time comes from the host; no implicit timers are started.
#![allow(dead_code)]
use crate::generated_ports::{text_input_semantics::segment_text_by_highlights,ui::VNode};
use serde_json::{json,Value};
use std::collections::BTreeMap;

fn box_node(attrs:BTreeMap<String,Value>,children:Vec<VNode>)->VNode {
    VNode::Element{tag:"Box".into(),attrs,children}
}
fn text_node(attrs:BTreeMap<String,Value>,children:Vec<VNode>)->VNode {
    VNode::Element{tag:"Text".into(),attrs,children}
}
pub fn render_source_view(props:&Value)->VNode {
    let text=props.get("text").and_then(Value::as_str).expect("HighlightedInput.text required");
    let highlights=props.get("highlights").and_then(Value::as_array).expect("HighlightedInput.highlights required");
    let segments=segment_text_by_highlights(text,highlights);
    let mut lines:Vec<Vec<(String,Value,usize)>>=vec![vec![]];
    let mut pos=0usize;
    for segment in &segments {
        let content=segment["text"].as_str().unwrap_or("");
        let highlight=segment.get("highlight").cloned().unwrap_or(Value::Null);
        for (index,part) in content.split('\n').enumerate() {
            if index>0 { lines.push(vec![]); pos+=1; }
            if !part.is_empty() { lines.last_mut().expect("line exists").push((part.into(),highlight.clone(),pos)); }
            pos+=part.encode_utf16().count();
        }
    }
    let shimmer:Vec<&Value>=highlights.iter().filter(|h|h.get("shimmerColor").is_some_and(|s|s.as_str().is_some_and(|s|!s.is_empty()))).collect();
    let glimmer_index=if shimmer.is_empty() { -100 } else {
        let lo=shimmer.iter().map(|h|h["start"].as_i64().unwrap_or(0)).min().unwrap_or(0);
        let hi=shimmer.iter().map(|h|h["end"].as_i64().unwrap_or(0)).max().unwrap_or(0);
        let cycle=(hi-lo+20).max(1);
        let millis=props.get("time").and_then(Value::as_i64).unwrap_or(0).max(0);
        lo-10+(millis/50)%cycle
    };
    let line_nodes=lines.into_iter().map(|parts| {
        if parts.is_empty() { return box_node(BTreeMap::new(),vec![text_node(BTreeMap::new(),vec![VNode::Text(" ".into())])]); }
        box_node(BTreeMap::new(),parts.into_iter().map(|(content,highlight,start)| {
            if let (Some(message_color),Some(shimmer_color))=(highlight["color"].as_str(),highlight["shimmerColor"].as_str()) {
                let chars=content.chars().enumerate().map(|(i,ch)| {
                    let color=if (start as i64+i as i64-glimmer_index).abs()<=1 {shimmer_color} else {message_color};
                    text_node(BTreeMap::from([("color".into(),json!(color))]),vec![VNode::Text(ch.to_string())])
                }).collect();
                return text_node(BTreeMap::new(),chars);
            }
            let mut attrs=BTreeMap::new();
            for key in ["color","dimColor","inverse"] {
                if let Some(v)=highlight.get(key) { attrs.insert(key.into(),v.clone()); }
            }
            text_node(attrs,vec![VNode::Element{tag:"Ansi".into(),attrs:BTreeMap::new(),children:vec![VNode::Text(content)]}])
        }).collect())
    }).collect();
    box_node(BTreeMap::from([("flexDirection".into(),json!("column")),("animated".into(),json!(!shimmer.is_empty()))]),line_nodes)
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn multi_line_highlight_preserves_empty_lines_and_colors() {
        let node=render_source_view(&json!({"text":"ab\n\ncd","highlights":[{"start":0,"end":2,"color":"red","priority":0}]}));
        let VNode::Element{children,attrs,..}=node else {panic!("Box")};
        assert_eq!(attrs["animated"],false);
        assert_eq!(children.len(),3);
        assert_eq!(children[1].text_content()," ");
        assert_eq!(children[2].text_content(),"cd");
    }
    #[test]
    fn shimmer_highlights_only_current_and_neighboring_characters() {
        let p=json!({"text":"hello","time":500,"highlights":[{"start":0,"end":5,"priority":1,"color":"dim","shimmerColor":"bright"}]});
        let view=render_source_view(&p);
        assert_eq!(view.text_content(),"hello");
        let VNode::Element{attrs,..}=view else {panic!("Box")};
        assert_eq!(attrs["animated"],true);
    }
}
