//! Generic shell argument tokenizer and injection checks, adapted from the
//! original utils/bash/shellQuote.ts. This parser does not execute commands.
use anyhow::{bail,Result};
use serde_json::Value;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ParseEntry { Word(String), Operator(String), Comment(String) }

pub fn parse(command:&str)->Result<Vec<ParseEntry>>{
    let chars=command.chars().collect::<Vec<_>>();
    let (mut result,mut current)=(Vec::new(),String::new());
    let (mut active,mut single,mut double)=(false,false,false);
    let mut i=0;
    while i<chars.len(){
        let ch=chars[i];
        if single{
            if ch=='\''{single=false}else{current.push(ch)}
        }else if double{
            if ch=='"'{double=false}
            else if ch=='\\' && i+1<chars.len(){
                let next=chars[i+1];
                if matches!(next,'$'|'"'|'\\'|'\n'){
                    i+=1;if next!='\n'{current.push(next)}
                }else{current.push(ch)}
            }else{current.push(ch)}
        }else if ch=='\'' {single=true;active=true}
        else if ch=='"' {double=true;active=true}
        else if ch=='\\' {
            if i+1>=chars.len(){bail!("Dangling shell escape")}
            i+=1;
            if chars[i]!='\n'{current.push(chars[i]);active=true}
        }
        else if ch.is_whitespace(){
            if active{result.push(ParseEntry::Word(std::mem::take(&mut current)));active=false;}
        }
        else if ch=='#' && !active {
            result.push(ParseEntry::Comment(chars[i+1..].iter().collect()));
            break;
        }
        else if matches!(ch,'|'|'&'|';'|'<'|'>'|'('|')'){
            if active{result.push(ParseEntry::Word(std::mem::take(&mut current)));active=false;}
            let mut op=ch.to_string();
            if i+1<chars.len() && chars[i+1]==ch && matches!(ch,'|'|'&'|'<'|'>'){
                op.push(ch);i+=1;
            }
            result.push(ParseEntry::Operator(op));
        }else{current.push(ch);active=true}
        i+=1;
    }
    if single||double{bail!("Unterminated shell quotation")}
    if active{result.push(ParseEntry::Word(current))}
    Ok(result)
}
pub fn parse_words(command:&str)->Result<Vec<String>>{
    Ok(parse(command)?.into_iter().filter_map(|v|match v{
        ParseEntry::Word(word)=>Some(word),_=>None
    }).collect())
}
pub fn has_malformed_tokens(command:&str,tokens:&[ParseEntry])->bool{
    let (mut single,mut double)=(false,false);
    let mut quotes=[0usize;2];
    let chars=command.chars().collect::<Vec<_>>();
    let mut i=0;
    while i<chars.len(){
        match chars[i]{
            '\\' if !single=>{i+=2;continue}
            '"' if !single=>{quotes[0]+=1;double=!double}
            '\'' if !double=>{quotes[1]+=1;single=!single}
            _=>{}
        }
        i+=1;
    }
    if quotes[0]%2==1||quotes[1]%2==1{return true}
    for item in tokens{
        let ParseEntry::Word(word)=item else{continue};
        for (left,right) in [('{','}'),('(',')'),('[',']')]{
            if word.matches(left).count()!=word.matches(right).count(){return true}
        }
        let chars=word.chars().collect::<Vec<_>>();
        for quote in ['\'','"']{
            if chars.iter().enumerate().filter(|(i,c)|**c==quote &&
                (*i==0 || chars[*i-1]!='\\')).count()%2==1{return true}
        }
    }
    false
}
pub fn has_shell_quote_single_quote_bug(command:&str)->bool{
    let chars=command.chars().collect::<Vec<_>>();
    let (mut single,mut double)=(false,false);
    let mut i=0;
    while i<chars.len(){
        let ch=chars[i];
        if ch=='\\' && !single{i+=2;continue}
        if ch=='"'&&!single{double=!double;i+=1;continue}
        if ch=='\''&&!double{
            single=!single;
            if !single{
                let mut j=i;
                while j>0&&chars[j-1]=='\\'{j-=1}
                let count=i-j;
                if count%2==1{return true}
                if count>0&&chars[i+1..].contains(&'\''){return true}
            }
        }
        i+=1;
    }
    false
}

fn single_quote(value:&str)->String{
    if value.is_empty(){return "''".into()}
    if value.bytes().all(|b|b.is_ascii_alphanumeric()||
        matches!(b,b'_'|b'-'|b'.'|b'/'|b':'|b'@'|b'%'|b'+'|b'='|b',')){
        return value.into()
    }
    format!("'{}'",value.replace('\'',"'\\''"))
}
pub fn try_quote_shell_args(args:&[Value])->Result<String>{
    let mut quoted=Vec::new();
    for (index,arg) in args.iter().enumerate(){
        let value=match arg{
            Value::String(s)=>s.clone(),Value::Null=>"null".into(),
            Value::Bool(b)=>b.to_string(),Value::Number(n)=>n.to_string(),
            _=>bail!("Cannot quote argument at index {index}: object values are not supported"),
        };
        quoted.push(single_quote(&value));
    }
    Ok(quoted.join(" "))
}
pub fn quote(args:&[Value])->String{
    args.iter().map(|v|single_quote(&match v{
        Value::String(s)=>s.to_owned(),Value::Null=>"null".into(),
        _=>v.to_string()
    })).collect::<Vec<_>>().join(" ")
}
#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]
    fn parsing_respects_quoting_operators_and_literal_env(){
        assert_eq!(parse_words(r#"one 'two words' "three words" four\ five"#).unwrap(),
            vec!["one","two words","three words","four five"]);
        assert_eq!(parse_words("echo $HOME | cat; echo tail").unwrap(),
            vec!["echo","$HOME","cat","echo","tail"]);
        assert_eq!(parse_words("a # comment").unwrap(),vec!["a"]);
        assert_eq!(parse_words("'' \"\"").unwrap(),vec!["",""]);
        assert!(parse_words("echo 'not closed").is_err());
    }
    #[test]
    fn recognizes_source_parser_ambiguities(){
        assert!(has_malformed_tokens("echo 'unterminated",&[]));
        assert!(has_malformed_tokens("echo \"unterminated",&[]));
        assert!(has_malformed_tokens("echo {json",&[ParseEntry::Word("{json".into())]));
        assert!(!has_malformed_tokens("echo '{ok}'",&[ParseEntry::Word("{ok}".into())]));
        assert!(has_shell_quote_single_quote_bug(r"git ls-remote 'safe\' '--upload-pack=evil' repo"));
        assert!(!has_shell_quote_single_quote_bug("echo 'normal'"));
    }
    #[test]
    fn shell_quotes_never_leave_metacharacters_executable(){
        assert_eq!(try_quote_shell_args(&[json!("echo"),json!("$(whoami)"),json!(true)])
            .unwrap(),"echo '$(whoami)' true");
        assert!(try_quote_shell_args(&[json!({"x":1})]).is_err());
        assert_eq!(quote(&[json!(["$(whoami)"]),json!("a'b")]),
            "'[\"$(whoami)\"]' 'a'\\''b'");
    }
}
