//! Manual source-order view of `src/components/PromptInput/PromptInputFooterLeftSide.tsx`.
//! Props include terminal dimensions, the application task/team state, and
//! feature/configuration flags. Dynamic subscriptions must be provided by host.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn node(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {tag: tag.into(), attrs, children}
}
fn attr(names: &[(&str, Value)]) -> BTreeMap<String, Value> {
    names.iter().map(|(key,value)| ((*key).to_string(),value.clone())).collect()
}
fn bool_prop(props: &Value, key: &str) -> bool {props.get(key).and_then(Value::as_bool).unwrap_or(false)}
fn str_prop<'a>(props: &'a Value, key: &str, default: &'a str) -> &'a str {
    props.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn text(content: &str, dim: bool, color: Option<&str>) -> VNode {
    let mut attrs=attr(&[("dimColor",json!(dim))]);
    if let Some(color)=color {attrs.insert("color".into(),json!(color));}
    node("Text",attrs,vec![VNode::Text(content.into())])
}
fn shortcut(shortcut: &str, action: &str) -> VNode {
    node("KeyboardShortcutHint",attr(&[("shortcut",json!(shortcut)),("action",json!(action))]),vec![])
}
fn mode_config(mode:&str)->(&'static str,&'static str,&'static str) {
    match mode {
        "plan"=>("⏸","plan mode","planMode"),
        "acceptEdits"=>("⏵⏵","accept edits","autoAccept"),
        "bypassPermissions"=>("⏵⏵","bypass permissions","error"),
        "dontAsk"=>("⏵⏵","don't ask","error"),
        "auto"=>("⏵⏵","auto mode","warning"),
        _=>("","default","text"),
    }
}
fn spinner_hint_parts(props:&Value, has_teammates:bool, has_running_agents:bool)->Vec<VNode> {
    let mut result=Vec::new();
    let is_loading=bool_prop(props,"isLoading");
    let esc=str_prop(props,"escShortcut","esc").to_lowercase();
    if is_loading {result.push(shortcut(&esc,"interrupt"));}
    if !is_loading && has_running_agents && !bool_prop(props,"killAgentsConfirmShowing") {
        result.push(shortcut(str_prop(props,"killAgentsShortcut","ctrl+x ctrl+k"),"stop agents"));
    }
    if bool_prop(props,"hasTaskItems") || has_teammates {
        let expanded=str_prop(props,"expandedView","none");
        let action=if has_teammates {match expanded {
            "tasks"=>"show teammates", "teammates"=>"hide", _=>"show tasks"
        }} else if expanded=="tasks" {"hide tasks"} else {"show tasks"};
        result.push(shortcut(str_prop(props,"todosShortcut","ctrl+t"),action));
    }
    result
}
fn status_part(tag:&str,props:&Value,keys:&[&str])->VNode {
    let attrs=keys.iter().filter_map(|key|props.get(*key).map(|value|((*key).into(),value.clone()))).collect();
    node(tag,attrs,vec![])
}
fn render_mode_indicator(props:&Value)->VNode {
    if str_prop(props,"mode", "prompt")=="bash" {return text("! for bash mode",false,Some("bashBorder"));}
    let selected_mode=props.get("toolPermissionContext").and_then(|v|v.get("mode"))
        .and_then(Value::as_str).unwrap_or("default");
    let active_mode=selected_mode!="default";
    let is_coordinator=bool_prop(props,"isCoordinator");
    let viewing_teammate=bool_prop(props,"isViewingTeammate");
    let completed_teammate=bool_prop(props,"isViewingCompletedTeammate");
    let running=props.get("runningTaskCount").and_then(Value::as_u64).unwrap_or(0)>0;
    let has_background_tasks=running || viewing_teammate;
    let has_teams=bool_prop(props,"agentSwarmsEnabled") && !bool_prop(props,"inProcessEnabled")
        && props.get("nonLeaderTeammateCount").and_then(Value::as_u64).unwrap_or(0)>0;
    let primary=usize::from(is_coordinator || active_mode)+usize::from(has_background_tasks)+usize::from(has_teams);
    let columns=props.get("terminalColumns").and_then(Value::as_u64).unwrap_or(80);
    let show_pr=bool_prop(props,"prStatusEnabled")
        && props.get("prNumber").is_some_and(|v|!v.is_null())
        && props.get("prReviewState").is_some_and(|v|!v.is_null())
        && props.get("prUrl").is_some_and(|v|!v.is_null())
        && primary<2 && (primary==0 || columns>=80);
    let show_hint=bool_prop(props,"showHint");
    let spinner_tree=str_prop(props,"expandedView","none")=="teammates";
    let has_teammate_pills=!spinner_tree && has_background_tasks &&
        (bool_prop(props,"hasInProcessTeammates") || viewing_teammate);
    let current_mode=if active_mode && !bool_prop(props,"isRemoteMode") {
        let (symbol,title,color)=mode_config(selected_mode);
        let mut children=vec![VNode::Text(format!("{symbol} {title} on"))];
        if primary<2 {
            children.push(node("Text",attr(&[("dimColor",json!(true))]),vec![
                VNode::Text(" ".into()),
                node("KeyboardShortcutHint",attr(&[("shortcut",json!(str_prop(props,"modeCycleShortcut","shift+tab"))),
                    ("action",json!("cycle")),("parens",json!(true))]),vec![])]));
        }
        Some(node("Text",attr(&[("color",json!(color))]),children))
    } else {None};
    let mut parts:Vec<VNode>=Vec::new();
    if let Some(url)=props.get("remoteSessionUrl").and_then(Value::as_str).filter(|s|!s.is_empty()) {
        parts.push(node("Link",attr(&[("url",json!(url))]),vec![text("◎ remote",false,Some("ide"))]));
    }
    if has_teams {
        parts.push(status_part("TeamStatus",props,&["teamsSelected","showHint"]));
    }
    if show_pr {
        let mut attrs=BTreeMap::new();
        for (target,source) in [("number","prNumber"),("url","prUrl"),("reviewState","prReviewState")] {
            if let Some(value)=props.get(source){attrs.insert(target.into(),value.clone());}
        }
        parts.push(node("PrBadge",attrs,vec![]));
    }
    let has_teammates=bool_prop(props,"hasAnyInProcessTeammates");
    let has_running_agents=bool_prop(props,"hasRunningAgentTasks");
    let hint_parts=if show_hint {spinner_hint_parts(props,has_teammates,has_running_agents)} else {vec![]};
    if completed_teammate {parts.push(shortcut(str_prop(props,"escShortcut","esc"),"return to team lead"));}
    else if bool_prop(props,"proactiveEnabled") && props.get("nextTickAt").is_some_and(|v|!v.is_null()) {
        parts.push(status_part("ProactiveCountdown",props,&["nextTickAt","now"]));
    } else if !has_teammate_pills && show_hint {parts.extend(hint_parts.clone());}

    let task_pill=if has_background_tasks && !has_teammate_pills && !bool_prop(props,"hideTasksFooter") {
        Some(status_part("BackgroundTaskStatus",props,&[
            "tasksSelected","isViewingTeammate","teammateFooterIndex","isLeaderIdle"]))
    } else {None};
    if parts.is_empty() && task_pill.is_none() && current_mode.is_none() && show_hint {
        parts.push(text("? for shortcuts",true,None));
    }
    if bool_prop(props,"voiceEnabled") && bool_prop(props,"voiceWarmingUp") {
        parts.push(node("VoiceWarmupHint",BTreeMap::new(),vec![]));
    } else if bool_prop(props,"fullscreenEnabled") && bool_prop(props,"hasSelection")
        && (!bool_prop(props,"copyOnSelect") || bool_prop(props,"isXtermJs")) {
        let mut hints=vec![];
        if !bool_prop(props,"copyOnSelect") {hints.push(shortcut("ctrl+c","copy"));}
        if bool_prop(props,"isXtermJs") {
            if bool_prop(props,"isMacOS") && bool_prop(props,"altClickFailed") {
                hints.push(text("set macOptionClickForcesSelection in VS Code settings",false,None));
            } else { hints.push(shortcut(if bool_prop(props,"isMacOS") {"option+click"} else {"shift+click"},"native select")); }
        }
        parts.push(node("Text",attr(&[("dimColor",json!(true))]),vec![node("Byline",BTreeMap::new(),hints)]));
    } else if bool_prop(props,"voiceEnabled") && str_prop(props,"voiceState","idle")=="idle"
        && !parts.is_empty() && show_hint && hint_parts.is_empty() && bool_prop(props,"voiceHintUnderCap") {
        parts.push(text(&format!("hold {} to speak",str_prop(props,"voiceKeyShortcut","Space")),true,None));
    }
    if (task_pill.is_some() || bool_prop(props,"hasCoordinatorTasks")) && show_hint && !has_teams {
        parts.push(shortcut(if bool_prop(props,"tasksSelected") {"Enter"} else {"↓"},
            if bool_prop(props,"tasksSelected") {"view tasks"} else {"manage"}));
    }

    if has_teammate_pills {
        let mut other=Vec::new();
        if let Some(mode)=current_mode {other.push(mode);}
        other.extend(parts);
        if !completed_teammate {other.extend(hint_parts);}
        let mut children=vec![node("Box",BTreeMap::new(),vec![
            status_part("BackgroundTaskStatus",props,&["tasksSelected","isViewingTeammate",
                "teammateFooterIndex","isLeaderIdle"])
        ])];
        if !other.is_empty() {children.push(node("Box",BTreeMap::new(),vec![
            node("Byline",BTreeMap::new(),other)]));}
        return node("Box",attr(&[("flexDirection",json!("column"))]),children);
    }
    if parts.is_empty() && task_pill.is_none() && current_mode.is_none() {
        return if bool_prop(props,"fullscreenEnabled") {text(" ",false,None)} else {VNode::Empty};
    }
    let mut children=Vec::new();
    if let Some(mode)=current_mode {
        let mut chunk=vec![mode];
        if task_pill.is_some() || !parts.is_empty(){chunk.push(text(" · ",true,None));}
        children.push(node("Box",attr(&[("flexShrink",json!(0))]),chunk));
    }
    if let Some(pill)=task_pill {
        let mut chunk=vec![pill];
        if !parts.is_empty() {chunk.push(text(" · ",true,None));}
        children.push(node("Box",attr(&[("flexShrink",json!(0))]),chunk));
    }
    if !parts.is_empty(){children.push(node("Text",attr(&[("wrap",json!("truncate"))]),vec![
        node("Byline",BTreeMap::new(),parts)
    ]));}
    node("Box",attr(&[("height",json!(1)),("overflow",json!("hidden"))]),children)
}

pub fn render_source_view(props:&Value)->VNode {
    if let Some(exit)=props.get("exitMessage") {
        if bool_prop(exit,"show") {
            let key=exit.get("key").and_then(Value::as_str).unwrap_or("undefined");
            return text(&format!("Press {key} again to exit"),true,None);
        }
    }
    if bool_prop(props,"isPasting") {return text("Pasting text…",true,None);}
    let searching=bool_prop(props,"isSearching");
    let show_vim=bool_prop(props,"vimModeEnabled") && str_prop(props,"vimMode","")=="INSERT" && !searching;
    let mut children=Vec::new();
    if searching {
        let mut attrs=BTreeMap::new();
        attrs.insert("value".into(),json!(str_prop(props,"historyQuery","")));
        attrs.insert("historyFailedMatch".into(),json!(bool_prop(props,"historyFailedMatch")));
        children.push(node("HistorySearchInput",attrs,vec![]));
    }
    if show_vim {children.push(text("-- INSERT --",true,None));}
    let mut attrs=BTreeMap::new();
    for name in ["mode","toolPermissionContext","isLoading","tasksSelected","teamsSelected",
        "tmuxSelected","teammateFooterIndex","onOpenTasksDialog","terminalColumns",
        "remoteSessionUrl","isRemoteMode","expandedView","runningTaskCount",
        "nonLeaderTeammateCount","agentSwarmsEnabled","inProcessEnabled","hasTaskItems",
        "hasInProcessTeammates","hasAnyInProcessTeammates","hasRunningAgentTasks",
        "isViewingTeammate","isViewingCompletedTeammate","hideTasksFooter","prStatusEnabled",
        "prNumber","prUrl","prReviewState","nextTickAt","proactiveEnabled","isCoordinator",
        "fullscreenEnabled","hasSelection","copyOnSelect","isXtermJs","isMacOS",
        "altClickFailed","voiceEnabled","voiceWarmingUp","voiceState","voiceHintUnderCap",
        "voiceKeyShortcut","hasCoordinatorTasks"] {
        if let Some(value)=props.get(name){attrs.insert(name.into(),value.clone());}
    }
    attrs.insert("showHint".into(),json!(!bool_prop(props,"suppressHint") && !show_vim));
    children.push(render_mode_indicator(&Value::Object(attrs.into_iter().collect())));
    node("Box",attr(&[("justifyContent",json!("flex-start")),("gap",json!(1))]),children)
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn exit_pasting_search_and_vim_precedence(){
        let mut p=json!({"mode":"prompt","exitMessage":{"show":true,"key":"ctrl+c"},"isPasting":true});
        assert_eq!(render_source_view(&p).text_content(),"Press ctrl+c again to exit");
        p["exitMessage"]["show"]=json!(false);
        assert_eq!(render_source_view(&p).text_content(),"Pasting text…");
        p["isPasting"]=json!(false);p["vimModeEnabled"]=json!(true);p["vimMode"]=json!("INSERT");
        assert!(render_source_view(&p).text_content().contains("-- INSERT --"));
        p["isSearching"]=json!(true);
        let VNode::Element {children,..}=render_source_view(&p) else {panic!("box")};
        assert!(!children.iter().any(|child|child.text_content().contains("-- INSERT --")));
        let VNode::Element {tag,..}=&children[0] else {panic!("search")};
        assert_eq!(tag,"HistorySearchInput");
    }
    #[test]
    fn mode_team_and_running_task_layout_do_not_nest_box_inside_text(){
        let view=render_mode_indicator(&json!({"mode":"prompt","toolPermissionContext":{"mode":"plan"},
            "runningTaskCount":2,"hasInProcessTeammates":true,"hasTaskItems":true,"isLoading":true,
            "expandedView":"none","showHint":true}));
        let VNode::Element {attrs,children,..}=view else {panic!("outer")};
        assert_eq!(attrs["flexDirection"],"column");
        assert_eq!(children.len(),2);
        assert!(view_tree_has_tag(&children[0],"BackgroundTaskStatus"));
    }
    fn view_tree_has_tag(node:&VNode,tag:&str)->bool {
        match node {VNode::Element{tag:node_tag,children,..}=>node_tag==tag || children.iter().any(|c|view_tree_has_tag(c,tag)),
            VNode::Group(children)=>children.iter().any(|c|view_tree_has_tag(c,tag)),_=>false}
    }
    #[test]
    fn bash_and_fullscreen_idle_reserve_one_row(){
        assert_eq!(render_mode_indicator(&json!({"mode":"bash"})).text_content(),"! for bash mode");
        assert_eq!(render_mode_indicator(&json!({"mode":"prompt","fullscreenEnabled":true})).text_content()," ");
        assert_eq!(render_mode_indicator(&json!({"mode":"prompt"})),VNode::Empty);
    }
}
