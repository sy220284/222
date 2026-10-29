//! Terminal teammate tree from src/components/Spinner/TeammateSpinnerTree.tsx.
//! The host passes the current AppState selectors (`tasks`,
//! `viewingAgentTaskId`, `showTeammateMessagePreview`) alongside component
//! props. Unlike the former static JSX stub, these values drive every row.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Map, Value};

fn element(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    let attrs = attrs
        .as_object()
        .expect("static teammate tree attributes")
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    VNode::Element {
        tag: tag.to_owned(),
        attrs,
        children,
    }
}

fn text(content: impl Into<String>, attrs: Value) -> VNode {
    element("Text", attrs, vec![VNode::Text(content.into())])
}

fn count_text(count: u64) -> String {
    let s = count.to_string();
    let mut out = String::new();
    for (index, c) in s.chars().enumerate() {
        if index > 0 && (s.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn running_teammates(props: &Value) -> Vec<Value> {
    let mut tasks = props
        .get("tasks")
        .and_then(Value::as_object)
        .map(Map::values)
        .into_iter()
        .flatten()
        .filter(|task| {
            task.get("type").and_then(Value::as_str) == Some("in_process_teammate")
                && task.get("status").and_then(Value::as_str) == Some("running")
        })
        .cloned()
        .collect::<Vec<_>>();
    tasks.sort_by(|a, b| {
        let a = a
            .pointer("/identity/agentName")
            .and_then(Value::as_str)
            .unwrap_or("");
        let b = b
            .pointer("/identity/agentName")
            .and_then(Value::as_str)
            .unwrap_or("");
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    tasks
}

/// Render a live teammate tree. `tasks` is the AppState task map; no task is
/// inferred from the data if the host supplies an empty or missing selector.
pub fn render_source_view(props: &Value) -> VNode {
    let teammates = running_teammates(props);
    if teammates.is_empty() {
        return VNode::Empty;
    }
    let selecting = props["isInSelectionMode"].as_bool().unwrap_or(false);
    let selected_index = props["selectedIndex"].as_i64();
    let leader_selected = selecting && selected_index == Some(-1);
    let leader_foregrounded = props.get("viewingAgentTaskId").is_none();
    let leader_highlighted = leader_foregrounded || leader_selected;
    let color = if leader_selected {
        "suggestion"
    } else {
        "cyan_FOR_SUBAGENTS_ONLY"
    };
    let mut leader = vec![
        text(
            if leader_selected { "❯" } else { " " },
            json!({"bold":leader_highlighted,"color":if leader_selected {Value::String("suggestion".into())} else {Value::Null}}),
        ),
        text(
            if leader_highlighted {
                "╒═ "
            } else {
                "┌─ "
            },
            json!({"dimColor":!leader_highlighted,"bold":leader_highlighted}),
        ),
        text(
            "team-lead",
            json!({"bold":leader_highlighted,"color":color}),
        ),
    ];
    if !leader_foregrounded {
        if let Some(verb) = props["leaderVerb"].as_str().filter(|s| !s.is_empty()) {
            leader.push(text(format!(": {verb}…"), json!({"dimColor":true})));
        } else if let Some(idle) = props["leaderIdleText"].as_str().filter(|s| !s.is_empty()) {
            leader.push(text(format!(": {idle}"), json!({"dimColor":true})));
        }
    }
    if let Some(tokens) = props["leaderTokenCount"].as_u64().filter(|n| *n > 0) {
        leader.push(text(
            format!(" · {} tokens", count_text(tokens)),
            json!({"dimColor":!leader_highlighted}),
        ));
    }
    if leader_highlighted {
        leader.push(text(" · shift + ↑/↓ to select", json!({"dimColor":true})));
    }
    if leader_selected && !leader_foregrounded {
        leader.push(text(" · enter to view", json!({"dimColor":true})));
    }
    let mut children = vec![element("Box", json!({"paddingLeft":3}), leader)];
    let viewer = props["viewingAgentTaskId"].as_str();
    for (index, teammate) in teammates.iter().enumerate() {
        let id = teammate["id"].as_str().unwrap_or_default();
        children.push(element(
            "TeammateSpinnerLine",
            json!({
                "key":id,
                "teammate":teammate,
                "isLast":!selecting && index+1==teammates.len(),
                "isSelected":selecting && selected_index==Some(index as i64),
                "isForegrounded":viewer==Some(id),
                "allIdle":props["allIdle"],
                "showPreview":props["showTeammateMessagePreview"]
            }),
            vec![],
        ));
    }
    if selecting {
        let is_hide_selected = selected_index == Some(teammates.len() as i64);
        let row = vec![
            text(
                if is_hide_selected { "❯" } else { " " },
                json!({"bold":is_hide_selected,"color":if is_hide_selected {Value::String("suggestion".into())} else {Value::Null}}),
            ),
            text(
                if is_hide_selected {
                    "╘═ "
                } else {
                    "└─ "
                },
                json!({"dimColor":!is_hide_selected,"bold":is_hide_selected}),
            ),
            text(
                "hide",
                json!({"dimColor":!is_hide_selected,"bold":is_hide_selected}),
            ),
        ];
        let mut row = row;
        if is_hide_selected {
            row.push(text(" · enter to collapse", json!({"dimColor":true})));
        }
        children.push(element("Box", json!({"paddingLeft":3}), row));
    }
    element(
        "Box",
        json!({"flexDirection":"column","marginTop":1}),
        children,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hides_tree_without_running_in_process_teammates() {
        assert_eq!(render_source_view(&json!({})), VNode::Empty);
        assert_eq!(
            render_source_view(
                &json!({"tasks":{"x":{"type":"in_process_teammate","status":"completed"}}})
            ),
            VNode::Empty
        );
    }
    #[test]
    fn source_ordering_selection_and_live_app_state_reach_child_lines() {
        let tree = render_source_view(&json!({
            "tasks":{
                "b":{"id":"id-b","type":"in_process_teammate","status":"running","identity":{"agentName":"beta"}},
                "other":{"id":"unused","type":"local_agent","status":"running"},
                "a":{"id":"id-a","type":"in_process_teammate","status":"running","identity":{"agentName":"alpha"}}
            },
            "isInSelectionMode":true,"selectedIndex":1,
            "viewingAgentTaskId":"id-b","allIdle":true,"showTeammateMessagePreview":true,
            "leaderVerb":"thinking","leaderTokenCount":1200
        }));
        let VNode::Element {
            children, attrs, ..
        } = tree
        else {
            panic!("tree root missing")
        };
        assert_eq!(attrs.get("flexDirection"), Some(&json!("column")));
        assert_eq!(children.len(), 4);
        assert!(children[0]
            .text_content()
            .contains("team-lead: thinking… · 1,200 tokens"));
        let VNode::Element { attrs: alpha, .. } = &children[1] else {
            panic!("alpha line missing")
        };
        let VNode::Element { attrs: beta, .. } = &children[2] else {
            panic!("beta line missing")
        };
        assert_eq!(alpha.get("key"), Some(&json!("id-a")));
        assert_eq!(beta.get("key"), Some(&json!("id-b")));
        assert_eq!(beta.get("isSelected"), Some(&json!(true)));
        assert_eq!(beta.get("isForegrounded"), Some(&json!(true)));
        assert_eq!(beta.get("showPreview"), Some(&json!(true)));
        assert!(children[3].text_content().contains("hide"));
    }
}
