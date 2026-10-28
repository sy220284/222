//! Original slashCommandParsing.ts semantics, preserving literal-space
//! splitting (including repeated spaces) and the "(MCP)" second-word marker.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ParsedSlashCommand {
    pub command_name: String,
    pub args: String,
    pub is_mcp: bool,
}
pub fn parse_slash_command(input: &str) -> Option<ParsedSlashCommand> {
    // JavaScript trim() includes U+FEFF even though Rust's trim() does not.
    let trimmed=input.trim_matches(|c:char|c.is_whitespace() || c=='\u{feff}');
    let without_slash=trimmed.strip_prefix('/')?;
    let words:Vec<&str>=without_slash.split(' ').collect();
    if words.first().copied().unwrap_or("").is_empty(){return None}
    let is_mcp=words.get(1).copied()==Some("(MCP)");
    let name=if is_mcp{
        format!("{} (MCP)",words[0])
    }else{
        words[0].to_owned()
    };
    let args=words[if is_mcp {2}else{1}..].join(" ");
    Some(ParsedSlashCommand{command_name:name,args,is_mcp})
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn standard_mcp_and_extra_spaces_match_original(){
        assert_eq!(parse_slash_command("/search foo bar"),Some(ParsedSlashCommand{
            command_name:"search".into(),args:"foo bar".into(),is_mcp:false}));
        assert_eq!(parse_slash_command(" /mcp:tool (MCP) arg1 arg2 "),Some(ParsedSlashCommand{
            command_name:"mcp:tool (MCP)".into(),args:"arg1 arg2".into(),is_mcp:true}));
        assert_eq!(parse_slash_command("/foo  bar"),Some(ParsedSlashCommand{
            command_name:"foo".into(),args:" bar".into(),is_mcp:false}));
        assert_eq!(parse_slash_command("/foo  (MCP) bar"),Some(ParsedSlashCommand{
            command_name:"foo".into(),args:" (MCP) bar".into(),is_mcp:false}));
        assert_eq!(parse_slash_command("/foo (MCP)"),Some(ParsedSlashCommand{
            command_name:"foo (MCP)".into(),args:"".into(),is_mcp:true}));
    }
    #[test]
    fn invalid_inputs_and_literal_space_behavior(){
        for invalid in ["","abc","/","/    ","   /  foo"]{
            assert_eq!(parse_slash_command(invalid),None,"{invalid}");
        }
        let value=parse_slash_command("\u{feff}/model  x  y\u{feff}").unwrap();
        assert_eq!(value.command_name,"model");
        assert_eq!(value.args," x  y");
        assert_eq!(parse_slash_command("/cmd\tx").unwrap().command_name,"cmd\tx");
    }
}
