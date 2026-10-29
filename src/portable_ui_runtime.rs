//! Provider-neutral rewrites of terminal, display, SDK and environment helpers.
//! Functions are separated from final feature parity while the large React/Ink
//! command surface is being reconstructed in Rust.
#![allow(dead_code)]
use std::{collections::{BTreeMap,HashMap},path::{Path,PathBuf},sync::{Arc,Mutex}};
use serde_json::{Value,json};

// src/utils/displayTags.ts — original JS regex uses a backreference; Rust's
// regex crate does not support it, so match a closing tag for the *same* name.
fn is_tag_start(c:char,index:usize)->bool{
    if index==0 { c.is_ascii_lowercase() }
    else {c.is_ascii_alphanumeric()||c=='_'||c=='-'}
}
fn remove_tag_blocks(text:&str,filter:impl Fn(&str)->bool)->String{
    let mut output=String::new();
    let mut index=0;
    while index<text.len(){
        let remaining=&text[index..];
        if remaining.starts_with('<'){
            let header=&remaining[1..];
            let name_len=header.chars().take_while(|c|is_tag_start(*c,0)).map(char::len_utf8).sum::<usize>();
            // Subsequent tag-name characters may include digits, '_', '-'.
            let name_len=if name_len>0 {header.char_indices().skip(1)
                .take_while(|(_,ch)|is_tag_start(*ch,1))
                .last().map(|(i,ch)|i+ch.len_utf8()).unwrap_or(name_len)} else {0};
            if name_len>0 {
                let tag=&header[..name_len];
                let after=&header[name_len..];
                let valid_header=after.starts_with('>')||after.chars().next().is_some_and(char::is_whitespace);
                if valid_header&&filter(tag){
                    if let Some(gt)=after.find('>'){
                        let open_len=1+name_len+gt+1;
                        let close=format!("</{tag}>");
                        if let Some(end)=remaining[open_len..].find(&close){
                            let mut skip=open_len+end+close.len();
                            if remaining[skip..].starts_with('\n'){skip+=1}
                            index+=skip;continue;
                        }
                    }
                }
            }
        }
        let ch=remaining.chars().next().unwrap();
        output.push(ch); index+=ch.len_utf8();
    }
    output
}
pub fn strip_display_tags_allow_empty(text:&str)->String {
    remove_tag_blocks(text,|_|true).trim().to_owned()
}
pub fn strip_display_tags(text:&str)->String {
    let clean=strip_display_tags_allow_empty(text);
    if clean.is_empty(){text.into()}else{clean}
}
pub fn strip_ide_context_tags(text:&str)->String{
    remove_tag_blocks(text,|tag|matches!(tag,"ide_opened_file"|"ide_selection")).trim().to_owned()
}

// src/utils/hyperlink.ts; OSC-8 wrapped in ANSI blue when supported.
pub const OSC8_START:&str="\x1b]8;;";
pub const OSC8_END:&str="\x07";
pub fn create_hyperlink(url:&str,text:Option<&str>,supports:bool)->String{
    if !supports{return url.into()}
    let display=text.unwrap_or(url);
    format!("{OSC8_START}{url}{OSC8_END}\x1b[34m{display}\x1b[39m{OSC8_START}{OSC8_END}")
}

// src/utils/privacyLevel.ts — no vendor telemetry is ever enabled by default.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PrivacyLevel{NoTelemetry,EssentialTraffic}
pub fn privacy_level(env:&HashMap<String,String>)->PrivacyLevel{
    if env.get("ATLAS_DISABLE_NONESSENTIAL_TRAFFIC").is_some_and(|v|!v.is_empty()){
        PrivacyLevel::EssentialTraffic
    } else {PrivacyLevel::NoTelemetry}
}
pub fn essential_traffic_only(env:&HashMap<String,String>)->bool{
    privacy_level(env)==PrivacyLevel::EssentialTraffic
}
pub fn telemetry_disabled(_env:&HashMap<String,String>)->bool{true}

// src/utils/gitSettings.ts
pub fn should_include_git_instructions(env:Option<&str>,setting:Option<bool>)->bool{
    match env.map(|v|v.trim().to_ascii_lowercase()){
        Some(v) if matches!(v.as_str(),"1"|"true"|"yes"|"on")=>false,
        Some(v) if matches!(v.as_str(),"0"|"false"|"no"|"off")=>true,
        _=>setting.unwrap_or(true),
    }
}

// src/utils/model/modelSupportOverrides.ts — custom-provider equivalents.
#[derive(Clone,Debug)]
pub struct ModelCapabilityTier {
    pub pinned_model_env:String,
    pub supported_capabilities_env:String
}
pub fn third_party_model_capability_override(
    provider_kind:&str,model:&str,capability:&str,
    tiers:&[ModelCapabilityTier],env:&HashMap<String,String>
)->Option<bool>{
    if provider_kind=="first-party"{return None}
    for tier in tiers{
        let (Some(pinned),Some(caps))=(env.get(&tier.pinned_model_env),
            env.get(&tier.supported_capabilities_env)) else{continue};
        if pinned.eq_ignore_ascii_case(model){
            return Some(caps.split(',').any(|v|v.trim().eq_ignore_ascii_case(capability)));
        }
    }
    None
}

// src/utils/task/sdkProgress.ts
pub fn task_progress_event(
    task_id:&str,tool_use_id:Option<&str>,description:&str,start_ms:i64,
    now_ms:i64,total_tokens:u64,tool_uses:u64,last_tool_name:Option<&str>,
    summary:Option<&str>,workflow:Option<Value>
)->Value{
    json!({
        "type":"system","subtype":"task_progress",
        "task_id":task_id,"tool_use_id":tool_use_id,"description":description,
        "usage":{"total_tokens":total_tokens,"tool_uses":tool_uses,
                 "duration_ms":now_ms-start_ms},
        "last_tool_name":last_tool_name,"summary":summary,
        "workflow_progress":workflow
    })
}

// src/utils/swarm/constants.ts — generic, non-vendor process labels.
pub const TEAM_LEAD_NAME:&str="team-lead";
pub const SWARM_SESSION_NAME:&str="atlas-swarm";
pub const SWARM_VIEW_WINDOW_NAME:&str="swarm-view";
pub const TMUX_COMMAND:&str="tmux";
pub const HIDDEN_SESSION_NAME:&str="atlas-hidden";
pub const TEAMMATE_COMMAND_ENV_VAR:&str="ATLAS_TEAMMATE_COMMAND";
pub const TEAMMATE_COLOR_ENV_VAR:&str="ATLAS_AGENT_COLOR";
pub const PLAN_MODE_REQUIRED_ENV_VAR:&str="ATLAS_PLAN_MODE_REQUIRED";
pub fn get_swarm_socket_name(pid:u32)->String{format!("{SWARM_SESSION_NAME}-{pid}")}

// src/utils/embeddedTools.ts
pub fn has_embedded_search_tools(enabled:bool,entrypoint:&str)->bool{
    enabled && !matches!(entrypoint,"sdk-ts"|"sdk-py"|"sdk-cli"|"local-agent")
}
pub fn embedded_search_tools_binary_path()->Option<PathBuf>{
    std::env::current_exe().ok()
}

// src/utils/memory/types.ts
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MemoryType{User,Project,Local,Managed,AutoMem,TeamMem}
impl MemoryType{
    pub fn values(include_team:bool)->&'static [&'static str]{
        if include_team{&["User","Project","Local","Managed","AutoMem","TeamMem"]}
        else{&["User","Project","Local","Managed","AutoMem"]}
    }
}

// src/utils/memory/versions.ts: find Git root without spawning a subprocess.
pub fn find_git_root(path:&Path)->Option<PathBuf>{
    for dir in path.ancestors(){
        let git=dir.join(".git");
        if git.is_dir()||git.is_file(){return Some(dir.to_path_buf())}
    }
    None
}
pub fn project_is_git_repo(path:&Path)->bool{find_git_root(path).is_some()}

// src/utils/teleport/environmentSelection.ts: selection by ID and source
// priority after environment retrieval (caller performs API operation).
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RemoteEnvironment{pub id:String,pub kind:String}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct EnvironmentSelection{
    pub environments:Vec<RemoteEnvironment>,
    pub selected:Option<RemoteEnvironment>,
    pub source:Option<String>,
}
pub fn select_remote_environment(
    environments:Vec<RemoteEnvironment>,
    default_id:Option<&str>,
    ordered_sources:&[(String,Option<String>)]
)->EnvironmentSelection{
    let fallback=environments.iter().find(|e|e.kind!="bridge")
        .or_else(||environments.first()).cloned();
    let requested=default_id.and_then(|id|environments.iter().find(|e|e.id==id)).cloned();
    let source=if requested.is_some(){
        ordered_sources.iter().rev()
            .find(|(name,value)|name!="flagSettings" && value.as_deref()==default_id)
            .map(|(name,_)|name.clone())
    }else{None};
    EnvironmentSelection{environments,selected:requested.or(fallback),source}
}

// src/utils/statusNoticeHelpers.ts: count only user-defined agent descriptions.
pub const AGENT_DESCRIPTIONS_THRESHOLD:usize=15_000;
pub fn agent_descriptions_total_tokens(
    agents:&[(String,String,String)],
    rough_token_count:impl Fn(&str)->usize
)->usize {
    agents.iter().filter(|(_,_,source)|source!="built-in")
        .map(|(kind,use_case,_)|rough_token_count(&format!("{kind}: {use_case}"))).sum()
}

// src/utils/autoModeDenials.ts: bounded most-recent-first in-memory state.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AutoModeDenial {
    pub tool_name:String,pub display:String,pub reason:String,pub timestamp_ms:i64
}
#[derive(Debug,Default,Clone)]
pub struct AutoModeDenials{entries:Vec<AutoModeDenial>}
impl AutoModeDenials {
    pub fn record(&mut self,item:AutoModeDenial){
        self.entries.insert(0,item);
        self.entries.truncate(20);
    }
    pub fn all(&self)->&[AutoModeDenial]{&self.entries}
}

// src/utils/systemPromptType.ts is implemented in portable_source_utils.rs.
// src/utils/settings/pluginOnlyPolicy.ts: generic permission read model.
pub fn plugin_enabled_by_policy(id:&str,settings:&BTreeMap<String,bool>)->bool{
    settings.get(id)!=Some(&false)
}

// src/utils/model/contextWindowUpgradeCheck.ts: multi-provider model
// capabilities, returning a command or tip when an expanded context exists.
#[derive(Clone,Debug)]
pub struct ModelContextUpgrade{
    pub alias:String,pub display_name:String,pub multiplier:u32
}
pub fn context_upgrade_message(upgrade:Option<&ModelContextUpgrade>,context:&str)->Option<String>{
    let upgrade=upgrade?;
    match context{
        "warning"=>Some(format!("/model {}",upgrade.alias)),
        "tip"=>Some(format!("Tip: You have access to {} with {}x more context",upgrade.display_name,upgrade.multiplier)),
        _=>None
    }
}

// src/utils/promptCategory.ts: local usage classification (no telemetry sink).
pub fn query_source_for_agent(name:Option<&str>,built_in:bool)->String {
    if built_in{if let Some(name)=name{format!("agent:builtin:{name}")}
        else{"agent:default".into()}}
    else {"agent:custom".into()}
}
pub fn query_source_for_repl(style:&str,default_style:&str,known_styles:&[&str])->String{
    if style==default_style{"repl_main_thread".into()}
    else if known_styles.contains(&style){format!("repl_main_thread:outputStyle:{style}")}
    else{"repl_main_thread:outputStyle:custom".into()}
}

// src/utils/keyboardShortcuts.ts: see portable_source_utils.rs.
// src/utils/cliHighlight.ts: extension to syntax language display fallback.
pub fn language_name_from_extension(path:&Path)->&'static str{
    match path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){
        "rs"=>"Rust","js"|"mjs"|"cjs"=>"JavaScript","ts"|"tsx"=>"TypeScript",
        "py"=>"Python","go"=>"Go","java"=>"Java","cpp"|"cc"|"cxx"=>"C++",
        "c"=>"C","cs"=>"C#","rb"=>"Ruby","json"=>"JSON","yaml"|"yml"=>"YAML",
        "md"=>"Markdown","toml"=>"TOML","sh"|"bash"=>"Bash",
        "html"=>"HTML","css"=>"CSS","sql"=>"SQL",_=>"unknown"
    }
}

// src/utils/swarm/teammateModel.ts: provider-neutral fallback to configured
// active/default model, never a hardcoded vendor-owned model identifier.
pub fn teammate_default_model(configured:Option<&str>,active:&str)->String{
    configured.filter(|s|!s.trim().is_empty()).unwrap_or(active).to_owned()
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn tag_matching_preserves_user_markup(){
        assert_eq!(strip_display_tags("hi <ide_opened_file>x</ide_opened_file>\nworld"),"hi world");
        assert_eq!(strip_display_tags("<ignored>only</ignored>"),"<ignored>only</ignored>");
        assert_eq!(strip_ide_context_tags("<code>keep</code><ide_selection>remove</ide_selection>"),"<code>keep</code>");
        assert_eq!(strip_display_tags("fix <Button> layout"),"fix <Button> layout");
        assert_eq!(strip_display_tags_allow_empty("<a>x</a><a>y</a>"),"");
        assert_eq!(strip_display_tags_allow_empty("<a>x</b>"),"<a>x</b>");
    }
    #[test]
    fn environment_selection_and_sdk_progress(){
        let a=RemoteEnvironment{id:"bridge".into(),kind:"bridge".into()};
        let b=RemoteEnvironment{id:"dev".into(),kind:"cloud".into()};
        let result=select_remote_environment(vec![a,b.clone()],Some("dev"),&[
            ("user".into(),Some("dev".into())),("project".into(),Some("dev".into()))]);
        assert_eq!(result.selected,Some(b));
        assert_eq!(result.source.as_deref(),Some("project"));
        let event=task_progress_event("task",Some("tool"),"desc",100,450,75,2,None,None,None);
        assert_eq!(event["usage"]["duration_ms"],350);
        assert_eq!(event["usage"]["total_tokens"],75);
        assert_eq!(context_upgrade_message(None,"tip"),None);
        assert!(create_hyperlink("https://a",Some("a"),true).contains(OSC8_START));
        assert_eq!(create_hyperlink("https://a",Some("a"),false),"https://a");
    }
    #[test]
    fn bounded_denials_and_capabilities(){
        let mut state=AutoModeDenials::default();
        for i in 0..22 { state.record(AutoModeDenial{
            tool_name:"Bash".into(),display:"cmd".into(),reason:"denied".into(),timestamp_ms:i
        })}
        assert_eq!(state.all().len(),20);
        assert_eq!(state.all()[0].timestamp_ms,21);
        assert_eq!(query_source_for_agent(Some("explore"),true),"agent:builtin:explore");
        assert_eq!(privacy_level(&HashMap::new()),PrivacyLevel::NoTelemetry);
        assert!(has_embedded_search_tools(true,"repl"));
        assert!(!has_embedded_search_tools(true,"sdk-ts"));
    }
}