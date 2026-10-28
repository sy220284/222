//! Independent implementations of simple source-level compatibility contracts.
use regex::{Captures, Regex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    pub expanded: String,
    pub missing_vars: Vec<String>,
}

pub fn expand_env_vars_with<F>(source: &str, mut lookup: F) -> Expanded
where F: FnMut(&str) -> Option<String> {
    let regex=Regex::new(r"\$\{([^}]+)\}").expect("fixed regex");
    let mut missing_vars=Vec::new();
    let expanded=regex.replace_all(source, |capture:&Captures<'_>| {
        let mut parts=capture[1].split(":-");
        let variable=parts.next().unwrap_or_default();
        let default=parts.next();
        if let Some(v)=lookup(variable) {v}
        else if let Some(v)=default {v.to_owned()}
        else {missing_vars.push(variable.to_owned());capture[0].to_owned()}
    }).into_owned();
    Expanded{expanded,missing_vars}
}
pub fn expand_env_vars_in_string(source:&str)->Expanded {
    expand_env_vars_with(source,|key|std::env::var(key).ok())
}
pub fn extract_bash_comment_label(command:&str)->Option<String> {
    fn whitespace(ch:char)->bool {ch.is_whitespace()||ch=='\u{feff}'}
    let first=command.split('\n').next().unwrap_or("").trim_matches(whitespace);
    if !first.starts_with('#')||first.starts_with("#!") {return None}
    let label=first.trim_start_matches('#').trim_start_matches(whitespace);
    if label.is_empty(){None}else{Some(label.to_owned())}
}
pub fn is_worktree_mode_enabled()->bool {true}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn source_env_expansion_cases(){
        let env=HashMap::from([("SET","yes"),("EMPTY","")]);
        let cases=[
            ("${SET}","yes",""),
            ("${EMPTY:-fallback}","",""),
            ("${MISSING:-default}","default",""),
            ("${MISSING:-one:-two}","one",""),
            ("${MISSING}","${MISSING}","MISSING"),
            ("${MISSING}/${MISSING}","${MISSING}/${MISSING}","MISSING,MISSING"),
            ("$SET","$SET",""),
            ("${}","${}",""),
            ("${SET}/${EMPTY}","yes/",""),
        ];
        for(input,expected,missing)in cases {
            let value=expand_env_vars_with(input,|k|env.get(k).map(|v|v.to_string()));
            assert_eq!(value.expanded,expected,"{input}");
            assert_eq!(value.missing_vars.join(","),missing,"{input}");
        }
    }
    #[test]
    fn source_bash_comment_cases(){
        let cases=[
            ("# explain\nprintf x",Some("explain")),
            ("###  work\nnext",Some("work")),
            ("#foo",Some("foo")),
            ("  # comment  ",Some("comment")),
            ("#\nhello",None),
            ("#!/bin/bash\nx",None),
            ("echo hi\n# comment",None),
            ("",None),
            ("#  ",None),
            (" #!not-a-comment",None),
            ("##\u{feff}heading",Some("heading")),
        ];
        for(input,expected)in cases {
            assert_eq!(extract_bash_comment_label(input).as_deref(),expected,"{input}");
        }
    }
    #[test]
    fn worktree_feature_is_enabled(){assert!(is_worktree_mode_enabled());}
}
