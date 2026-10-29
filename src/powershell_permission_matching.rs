//! PowerShell rule evaluation derived from
//! src/tools/PowerShellTool/powershellPermissions.ts:163-326 and
//! src/utils/powershell/parser.ts:814-826,1326-1451.
//! Only rule-content matching is implemented; AST danger checks remain open.
use crate::permission_rules::{Behavior,Rule,extract_prefix,has_wildcards,match_wildcard};
use anyhow::Result;

fn strip_module_prefix(value:&str)->&str{
    if value.len()>1 && value.as_bytes()[1]==b':' &&
        value.as_bytes()[0].is_ascii_alphabetic() ||
        value.starts_with(r"\\") || value.starts_with(r".\") || value.starts_with(r"..\") {
        return value;
    }
    value.rsplit_once('\\').map(|(_,x)|x).unwrap_or(value)
}
fn canonical_name(raw:&str)->String{
    let mut name=raw.to_lowercase();
    if !name.contains('\\')&&!name.contains('/') {
        for suffix in [".exe",".cmd",".bat",".com"] {
            if name.ends_with(suffix) {name.truncate(name.len()-suffix.len());break;}
        }
    }
    let canonical=match name.as_str(){
        "ls"|"dir"|"gci"=>"get-childitem",
        "cat"|"type"|"gc"=>"get-content",
        "cd"|"sl"|"chdir"=>"set-location",
        "pushd"=>"push-location","popd"=>"pop-location",
        "pwd"|"gl"=>"get-location",
        "gi"=>"get-item","gp"=>"get-itemproperty",
        "ni"|"mkdir"|"md"=>"new-item",
        "ri"|"del"|"rd"|"rmdir"|"rm"|"erase"=>"remove-item",
        "mi"|"mv"|"move"=>"move-item",
        "ci"|"cp"|"copy"|"cpi"=>"copy-item",
        "si"=>"set-item","rni"|"ren"=>"rename-item",
        "ps"|"gps"=>"get-process",
        "kill"|"spps"=>"stop-process",
        "start"|"saps"=>"start-process",
        "sajb"=>"start-job","ipmo"=>"import-module",
        "echo"|"write"=>"write-output","sleep"=>"start-sleep",
        "help"|"man"=>"get-help","gcm"=>"get-command",
        "gsv"=>"get-service","gv"=>"get-variable","sv"=>"set-variable",
        "h"|"history"=>"get-history",
        "iex"=>"invoke-expression","iwr"=>"invoke-webrequest",
        "irm"=>"invoke-restmethod","icm"=>"invoke-command",
        "ii"=>"invoke-item",
        "nsn"=>"new-pssession","etsn"=>"enter-pssession",
        "exsn"=>"exit-pssession","gsn"=>"get-pssession","rsn"=>"remove-pssession",
        "cls"|"clear"=>"clear-host",
        "select"=>"select-object","where"|"?"=>"where-object",
        "foreach"|"%"=>"foreach-object",
        "measure"=>"measure-object",
        "ft"=>"format-table","fl"=>"format-list","fw"=>"format-wide",
        "oh"=>"out-host","ogv"=>"out-gridview",
        "ac"=>"add-content","clc"=>"clear-content",
        "tee"=>"tee-object","epcsv"=>"export-csv",
        "sp"=>"set-itemproperty","rp"=>"remove-itemproperty",
        "cli"=>"clear-item","epal"=>"export-alias",
        "sls"=>"select-string",
        _=>return name,
    };
    canonical.to_owned()
}
fn split_command(cmd:&str)->(&str,&str){
    let split=cmd.find(char::is_whitespace).unwrap_or(cmd.len());
    let name=&cmd[..split];
    let rest=cmd[split..].trim_start();
    (name,rest)
}
fn make_command(cmd_name:&str,rest:&str)->String{
    if rest.is_empty(){cmd_name.to_string()}else{format!("{cmd_name} {rest}")}
}
fn match_one(pattern:&str,command:&str)->Result<bool>{
    if let Some(prefix)=extract_prefix(pattern){
        let p=prefix.to_lowercase();
        let cmd=command.to_lowercase();
        return Ok(cmd==p || cmd.starts_with(&format!("{p} ")));
    }
    if has_wildcards(pattern){return match_wildcard(pattern,command,true);}
    Ok(pattern.to_lowercase()==command.to_lowercase())
}
fn same_canonical(rule:&str,actual:&str,allow:bool)->Result<bool>{
    let (input_raw,input_rest)=split_command(actual);
    let input_name=canonical_name(strip_module_prefix(input_raw));
    let input=make_command(&input_name,input_rest);
    let prefix=extract_prefix(rule);
    let canonical_rule=prefix.unwrap_or(rule);
    let (rule_raw,rule_rest)=split_command(canonical_rule);
    let rule_name=canonical_name(if allow{rule_raw}else{strip_module_prefix(rule_raw)});
    if rule_name!=input_name{return Ok(false);}
    let rule_norm=make_command(&input_name,rule_rest);
    if prefix.is_some(){
        // The original prefix match accepts both bare cmdlet and cmdlet + args.
        let base=rule_norm.to_lowercase();
        let value=input.to_lowercase();
        return Ok(value==base||value.starts_with(&format!("{base} ")));
    }
    match_one(&rule_norm,&input)
}
fn matches_rule(rule:&str,actual:&str,allow:bool)->Result<bool>{
    // Original rule comparison first checks the raw command (case-insensitive).
    if match_one(rule,actual)?{return Ok(true);}
    let (raw,rest)=split_command(actual);
    let name=canonical_name(strip_module_prefix(raw));
    let normalized=make_command(&name,rest);
    if match_one(rule,&normalized)?{return Ok(true);}
    // Then the rule itself is canonicalised; deny/ask may strip its module
    // qualifier but allow may NOT (a cross-module fail-open).
    if extract_prefix(rule).is_some(){
        return same_canonical(rule,actual,allow);
    }
    if has_wildcards(rule){
        return same_canonical(rule,actual,allow);
    }
    same_canonical(rule,actual,allow)
}
pub fn policy_for(rules:&[Rule],command:&str)->Result<Option<Behavior>>{
    let actual=command.trim();
    let mut allow=false;
    let mut ask=false;
    for rule in rules {
        if rule.value.tool_name!="PowerShell"{continue;}
        let matched=match rule.value.rule_content.as_deref(){
            None=>true,
            Some(pattern)=>matches_rule(pattern,actual,rule.behavior==Behavior::Allow)?,
        };
        if !matched{continue;}
        match rule.behavior {
            Behavior::Deny=>return Ok(Some(Behavior::Deny)),
            Behavior::Ask=>ask=true,
            Behavior::Allow=>allow=true,
        }
    }
    Ok(if ask{Some(Behavior::Ask)}else if allow{Some(Behavior::Allow)}else{None})
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::permission_rules::{compile_rules,RuleSetting};
    fn compiled(rules:&[(&str,&str)])->Vec<Rule>{
        compile_rules(&rules.iter().map(|(behavior,rule)|RuleSetting{
            behavior:(*behavior).into(),rule:(*rule).into(),
        }).collect::<Vec<_>>()).unwrap()
    }
    #[test]
    fn raw_and_canonical_powershell_rules_do_not_see_bash(){
        let rules=compiled(&[("allow","Bash(Get-ChildItem:*)"),("deny","PowerShell(Remove-Item:*)")]);
        assert_eq!(policy_for(&rules,"rm secret").unwrap(),Some(Behavior::Deny));
        assert_eq!(policy_for(&rules,"REMove-ItEM secret").unwrap(),Some(Behavior::Deny));
        assert_eq!(policy_for(&rules,"rm\tsecret").unwrap(),Some(Behavior::Deny));
        assert_eq!(policy_for(&rules,"Get-ChildItem").unwrap(),None);
    }
    #[test]
    fn allow_alias_canonical_and_case_insensitivity(){
        let rules=compiled(&[("allow","PowerShell(Get-ChildItem:*)")]);
        assert_eq!(policy_for(&rules,"ls").unwrap(),Some(Behavior::Allow));
        assert_eq!(policy_for(&rules,"GCI -Recurse").unwrap(),Some(Behavior::Allow));
        assert_eq!(policy_for(&rules,"get-childitem\t-Recurse").unwrap(),Some(Behavior::Allow));
        assert_eq!(policy_for(&rules,"Get-ChildItemNot").unwrap(),None);
    }
    #[test]
    fn module_qualified_allow_is_not_cross_module_but_deny_is_fail_closed(){
        let rules=compiled(&[("allow",r"PowerShell(ModuleA\Get-Thing:*)")]);
        assert_eq!(policy_for(&rules,r"ModuleB\Get-Thing foo").unwrap(),None);
        let rules=compiled(&[("deny",r"PowerShell(ModuleA\Remove-Item:*)")]);
        assert_eq!(policy_for(&rules,"rm secret").unwrap(),Some(Behavior::Deny));
        let rules=compiled(&[("deny","PowerShell(rm *)")]);
        assert_eq!(policy_for(&rules,"Remove-Item secret").unwrap(),Some(Behavior::Deny));
    }
    #[test]
    fn deny_wins_and_native_exe_aliases_are_not_inferred(){
        let rules=compiled(&[
            ("allow","PowerShell(Remove-Item:*)"),
            ("ask","PowerShell(Remove-Item:*)"),
            ("deny","PowerShell(rm:*)"),
        ]);
        assert_eq!(policy_for(&rules,"Remove-Item secret").unwrap(),Some(Behavior::Deny));
        let rules=compiled(&[("allow","PowerShell(Get-ChildItem:*)")]);
        assert_eq!(policy_for(&rules,"sort test").unwrap(),None);
        assert_eq!(policy_for(&rules,r".\Get-ChildItem foo").unwrap(),None);
    }
}
