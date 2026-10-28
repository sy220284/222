//! Provider-neutral permission rule parser and shell-rule matcher.
//! Based on src/utils/permissions/permissionRuleParser.ts and shellRuleMatching.ts.
use anyhow::{bail,Result};
use regex::RegexBuilder;
use serde::{Deserialize,Serialize};
use serde_json::Value;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RuleValue{pub tool_name:String,pub rule_content:Option<String>}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Behavior{Allow,Deny,Ask}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct RuleSetting{pub behavior:String,pub rule:String}
#[derive(Clone,Debug)]
pub struct Rule{pub behavior:Behavior,pub value:RuleValue}

pub fn normalize_legacy_tool(name:&str)->&str{
    match name{
        "Task"=>"Agent","KillShell"=>"TaskStop",
        "AgentOutputTool"|"BashOutputTool"=>"TaskOutput",
        other=>other,
    }
}
pub fn legacy_names(name:&str)->Vec<&'static str>{
    match name{
        "Agent"=>vec!["Task"],
        "TaskStop"=>vec!["KillShell"],
        "TaskOutput"=>vec!["AgentOutputTool","BashOutputTool"],
        _=>Vec::new(),
    }
}
pub fn escape_content(s:&str)->String{
    s.replace('\\',"\\\\").replace('(',"\\(").replace(')',"\\)")
}
pub fn unescape_content(s:&str)->String{
    s.replace("\\(","(").replace("\\)",")").replace("\\\\","\\")
}
fn first_unescaped(s:&str,target:char)->Option<usize>{
    s.match_indices(target).find(|(index,_)|{
        s[..*index].bytes().rev().take_while(|c|*c==b'\\').count()%2==0
    }).map(|(index,_)|index)
}
fn last_unescaped(s:&str,target:char)->Option<usize>{
    s.match_indices(target).rev().find(|(index,_)|{
        s[..*index].bytes().rev().take_while(|c|*c==b'\\').count()%2==0
    }).map(|(index,_)|index)
}
pub fn parse_rule(s:&str)->RuleValue{
    let tool_only=||RuleValue{tool_name:normalize_legacy_tool(s).into(),rule_content:None};
    let Some(start)=first_unescaped(s,'(')else{return tool_only()};
    let Some(end)=last_unescaped(s,')')else{return tool_only()};
    if end<=start||end!=s.len()-1||start==0{return tool_only()}
    let tool_name=normalize_legacy_tool(&s[..start]).to_owned();
    let body=&s[start+1..end];
    if body.is_empty()||body=="*"{return RuleValue{tool_name,rule_content:None}}
    RuleValue{tool_name,rule_content:Some(unescape_content(body))}
}
pub fn format_rule(value:&RuleValue)->String{
    match value.rule_content.as_deref().filter(|s|!s.is_empty()){
        Some(content)=>format!("{}({})",value.tool_name,escape_content(content)),
        None=>value.tool_name.clone(),
    }
}
pub fn extract_prefix(rule:&str)->Option<&str>{
    rule.strip_suffix(":*").filter(|prefix|!prefix.is_empty())
}
pub fn has_wildcards(rule:&str)->bool{
    if rule.ends_with(":*"){return false}
    rule.match_indices('*').any(|(index,_)|
        rule[..index].bytes().rev().take_while(|c|*c==b'\\').count()%2==0)
}
pub fn match_wildcard(pattern:&str,command:&str,case_insensitive:bool)->Result<bool>{
    let p=pattern.trim();
    let mut regex=String::from("^");
    let mut chars=p.chars().peekable();
    let mut unescaped_stars=0usize;
    while let Some(ch)=chars.next(){
        if ch=='\\' {
            match chars.peek().copied(){
                Some('*')|Some('\\')=>{
                    let next=chars.next().expect("peek succeeded");
                    regex.push_str(&regex::escape(&next.to_string()));
                    continue;
                }
                _=>{}
            }
        }
        if ch=='*'{regex.push_str(".*");unescaped_stars+=1;}
        else {regex.push_str(&regex::escape(&ch.to_string()));}
    }
    if unescaped_stars==1 && regex.ends_with(" .*"){
        regex.truncate(regex.len()-3);
        regex.push_str("( .*)?");
    }
    regex.push('$');
    let compiled=RegexBuilder::new(&regex)
        .case_insensitive(case_insensitive).dot_matches_new_line(true).build()?;
    Ok(compiled.is_match(command))
}
pub fn matches_content(rule:&str,command:&str)->Result<bool>{
    if let Some(prefix)=extract_prefix(rule){
        return Ok(command==prefix ||
            command.strip_prefix(prefix).is_some_and(|rest|rest.starts_with(' ')))
    }
    if has_wildcards(rule){return match_wildcard(rule,command,false)}
    Ok(rule==command)
}
pub fn canonical_tool(name:&str)->&str{
    match name{
        "run_command"|"Bash"=>"Bash",
        "task_stop"|"TaskStop"|"KillShell"=>"TaskStop",
        "task_output"|"TaskOutput"|"AgentOutputTool"|"BashOutputTool"=>"TaskOutput",
        "spawn_agent"|"spawn_team"|"Agent"|"Task"=>"Agent",
        "write_file"|"Write"=>"Write",
        "edit_file"|"Edit"=>"Edit",
        other=>normalize_legacy_tool(other),
    }
}
pub fn compile_rules(settings:&[RuleSetting])->Result<Vec<Rule>>{
    settings.iter().map(|setting|{
        let behavior=match setting.behavior.as_str(){
            "allow"=>Behavior::Allow,"deny"=>Behavior::Deny,
            "ask"=>Behavior::Ask,other=>bail!("Unknown permission behavior: {other}")
        };
        let value=parse_rule(&setting.rule);
        if value.tool_name.trim().is_empty(){bail!("Empty permission tool name")}
        if setting.rule.len()>4096{bail!("Permission rule too long")}
        Ok(Rule{behavior,value})
    }).collect()
}
pub fn policy_for(rules:&[Rule],tool:&str,args:&Value)->Result<Option<Behavior>>{
    let name=canonical_tool(tool);
    let mut allow=false;
    let mut ask=false;
    for rule in rules{
        if canonical_tool(&rule.value.tool_name)!=name{continue}
        let matched=match rule.value.rule_content.as_deref(){
            None=>true,
            Some(pattern)=>{
                let argument=match name{
                    "Bash"=>args.get("command").and_then(Value::as_str),
                    "Write"|"Edit"=>args.get("path").and_then(Value::as_str),
                    "TaskStop"=>args.get("task_id").or_else(||args.get("shell_id")).and_then(Value::as_str),
                    _=>None,
                };
                match argument{Some(argument)=>matches_content(pattern,argument)?,None=>false}
            },
        };
        if !matched{continue}
        match rule.behavior{
            Behavior::Deny=>return Ok(Some(Behavior::Deny)),
            Behavior::Ask=>ask=true,
            Behavior::Allow=>allow=true,
        }
    }
    if ask{Ok(Some(Behavior::Ask))}
    else if allow{Ok(Some(Behavior::Allow))}
    else{Ok(None)}
}
#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]
    fn parsing_and_escaping_reference_cases(){
        for (text,name,content) in [
            ("Bash","Bash",None),
            ("Bash(npm install)","Bash",Some("npm install")),
            ("Bash(python -c \"print\\(1\\)\")","Bash",Some("python -c \"print(1)\"")),
            ("Bash(*)","Bash",None),
            ("Bash()","Bash",None),
            ("(foo)","(foo)",None),
            ("Bash(a)b","Bash(a)b",None),
            ("KillShell(abc)","TaskStop",Some("abc")),
            ("Task","Agent",None),
        ]{
            let parsed=parse_rule(text);
            assert_eq!(parsed.tool_name,name);
            assert_eq!(parsed.rule_content.as_deref(),content);
        }
        let values=["abc\\(hi\\)","x(a)","hey\\\\cmd","😀 (hello)"];
        for value in values{assert_eq!(unescape_content(&escape_content(value)),value);}
        assert_eq!(legacy_names("TaskOutput"),vec!["AgentOutputTool","BashOutputTool"]);
    }
    #[test]
    fn wildcard_and_legacy_prefix_match_reference_cases(){
        assert!(!has_wildcards("npm:*"));
        assert!(!has_wildcards(r"literal\*"));
        assert!(has_wildcards("git *"));
        for (pattern,command,matched) in [
            ("git *","git status",true),("git *","git",true),
            ("git *","gitstatus",false),
            ("npm:*","npm",true),("npm:*","npm run test",true),
            ("npm:*","npminstall",false),
            (r"echo \*","echo *",false),
            (r"echo \*","echo hey",false),
            ("npm * install","npm local install",true),
            ("npm * install","npm local\ninstall",false),
            ("npm * install","npm local\n install",true),
            ("foo","foobar",false),("foo","foo",true),
        ]{
            assert_eq!(matches_content(pattern,command).unwrap(),matched,"{pattern} vs {command}");
        }
        assert!(match_wildcard(r"echo \*","echo *",false).unwrap());
        assert!(match_wildcard("GIT *","git status",true).unwrap());
    }
    #[test]
    fn explicit_deny_precedes_allow_and_ask_overrides_allow(){
        let entries=compile_rules(&[
            RuleSetting{behavior:"allow".into(),rule:"Bash(git *)".into()},
            RuleSetting{behavior:"deny".into(),rule:"Bash(git push *)".into()},
            RuleSetting{behavior:"ask".into(),rule:"Bash(git reset *)".into()},
        ]).unwrap();
        assert_eq!(policy_for(&entries,"run_command",&json!({"command":"git status"})).unwrap(),Some(Behavior::Allow));
        assert_eq!(policy_for(&entries,"run_command",&json!({"command":"git push origin main"})).unwrap(),Some(Behavior::Deny));
        assert_eq!(policy_for(&entries,"run_command",&json!({"command":"git reset --hard"})).unwrap(),Some(Behavior::Ask));
        assert_eq!(policy_for(&entries,"run_command",&json!({"command":"ls"})).unwrap(),None);
        assert!(compile_rules(&[RuleSetting{behavior:"approve".into(),rule:"Bash".into()}]).is_err());
    }
}
