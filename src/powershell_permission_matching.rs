//! PowerShell rule evaluation derived from
//! src/tools/PowerShellTool/powershellPermissions.ts:163-326 and
//! src/utils/powershell/parser.ts:814-826,1326-1451.
//! Only rule-content matching is implemented; AST danger checks remain open.
use crate::permission_rules::{extract_prefix, has_wildcards, match_wildcard, Behavior, Rule};
use anyhow::Result;

fn strip_module_prefix(value: &str) -> &str {
    if value.len() > 1 && value.as_bytes()[1] == b':' && value.as_bytes()[0].is_ascii_alphabetic()
        || value.starts_with(r"\\")
        || value.starts_with(r".\")
        || value.starts_with(r"..\")
    {
        return value;
    }
    value.rsplit_once('\\').map(|(_, x)| x).unwrap_or(value)
}
fn canonical_name(raw: &str) -> String {
    let mut name = raw.to_lowercase();
    if !name.contains('\\') && !name.contains('/') {
        for suffix in [".exe", ".cmd", ".bat", ".com"] {
            if name.ends_with(suffix) {
                name.truncate(name.len() - suffix.len());
                break;
            }
        }
    }
    let canonical = match name.as_str() {
        "ls" | "dir" | "gci" => "get-childitem",
        "cat" | "type" | "gc" => "get-content",
        "cd" | "sl" | "chdir" => "set-location",
        "pushd" => "push-location",
        "popd" => "pop-location",
        "pwd" | "gl" => "get-location",
        "gi" => "get-item",
        "gp" => "get-itemproperty",
        "ni" | "mkdir" | "md" => "new-item",
        "ri" | "del" | "rd" | "rmdir" | "rm" | "erase" => "remove-item",
        "mi" | "mv" | "move" => "move-item",
        "ci" | "cp" | "copy" | "cpi" => "copy-item",
        "si" => "set-item",
        "rni" | "ren" => "rename-item",
        "ps" | "gps" => "get-process",
        "kill" | "spps" => "stop-process",
        "start" | "saps" => "start-process",
        "sajb" => "start-job",
        "ipmo" => "import-module",
        "echo" | "write" => "write-output",
        "sleep" => "start-sleep",
        "help" | "man" => "get-help",
        "gcm" => "get-command",
        "gsv" => "get-service",
        "gv" => "get-variable",
        "sv" => "set-variable",
        "h" | "history" => "get-history",
        "iex" => "invoke-expression",
        "iwr" => "invoke-webrequest",
        "irm" => "invoke-restmethod",
        "icm" => "invoke-command",
        "ii" => "invoke-item",
        "nsn" => "new-pssession",
        "etsn" => "enter-pssession",
        "exsn" => "exit-pssession",
        "gsn" => "get-pssession",
        "rsn" => "remove-pssession",
        "cls" | "clear" => "clear-host",
        "select" => "select-object",
        "where" | "?" => "where-object",
        "foreach" | "%" => "foreach-object",
        "measure" => "measure-object",
        "ft" => "format-table",
        "fl" => "format-list",
        "fw" => "format-wide",
        "oh" => "out-host",
        "ogv" => "out-gridview",
        "ac" => "add-content",
        "clc" => "clear-content",
        "tee" => "tee-object",
        "epcsv" => "export-csv",
        "sp" => "set-itemproperty",
        "rp" => "remove-itemproperty",
        "cli" => "clear-item",
        "epal" => "export-alias",
        "sls" => "select-string",
        _ => return name,
    };
    canonical.to_owned()
}
fn split_command(cmd: &str) -> (&str, &str) {
    let split = cmd.find(char::is_whitespace).unwrap_or(cmd.len());
    let name = &cmd[..split];
    let rest = cmd[split..].trim_start();
    (name, rest)
}
fn make_command(cmd_name: &str, rest: &str) -> String {
    if rest.is_empty() {
        cmd_name.to_string()
    } else {
        format!("{cmd_name} {rest}")
    }
}
fn match_one(pattern: &str, command: &str) -> Result<bool> {
    if let Some(prefix) = extract_prefix(pattern) {
        let p = prefix.to_lowercase();
        let cmd = command.to_lowercase();
        return Ok(cmd == p || cmd.starts_with(&format!("{p} ")));
    }
    if has_wildcards(pattern) {
        return match_wildcard(pattern, command, true);
    }
    Ok(pattern.to_lowercase() == command.to_lowercase())
}
fn same_canonical(rule: &str, actual: &str, allow: bool) -> Result<bool> {
    let (input_raw, input_rest) = split_command(actual);
    let input_name = canonical_name(strip_module_prefix(input_raw));
    let input = make_command(&input_name, input_rest);
    let prefix = extract_prefix(rule);
    let canonical_rule = prefix.unwrap_or(rule);
    let (rule_raw, rule_rest) = split_command(canonical_rule);
    let rule_name = canonical_name(if allow {
        rule_raw
    } else {
        strip_module_prefix(rule_raw)
    });
    if rule_name != input_name {
        return Ok(false);
    }
    let rule_norm = make_command(&input_name, rule_rest);
    if prefix.is_some() {
        // The original prefix match accepts both bare cmdlet and cmdlet + args.
        let base = rule_norm.to_lowercase();
        let value = input.to_lowercase();
        return Ok(value == base || value.starts_with(&format!("{base} ")));
    }
    match_one(&rule_norm, &input)
}
fn matches_rule(rule: &str, actual: &str, allow: bool) -> Result<bool> {
    // Original rule comparison first checks the raw command (case-insensitive).
    if match_one(rule, actual)? {
        return Ok(true);
    }
    let (raw, rest) = split_command(actual);
    let name = canonical_name(strip_module_prefix(raw));
    let normalized = make_command(&name, rest);
    if match_one(rule, &normalized)? {
        return Ok(true);
    }
    // Then the rule itself is canonicalised; deny/ask may strip its module
    // qualifier but allow may NOT (a cross-module fail-open).
    if extract_prefix(rule).is_some() {
        return same_canonical(rule, actual, allow);
    }
    if has_wildcards(rule) {
        return same_canonical(rule, actual, allow);
    }
    same_canonical(rule, actual, allow)
}
// Source: PowerShellTool/powershellPermissions.ts, especially
// getSubCommandsForPermissionCheck and the post-parse deny > ask > allow
// reduction. A rule for the first cmdlet must never authorize an entire
// pipeline or statement list.
fn policy_for_single(rules: &[Rule], command: &str) -> Result<Option<Behavior>> {
    let actual = command.trim();
    let mut allow = false;
    let mut ask = false;
    for rule in rules {
        if rule.value.tool_name != "PowerShell" {
            continue;
        }
        let matched = match rule.value.rule_content.as_deref() {
            None => true,
            Some(pattern) => matches_rule(pattern, actual, rule.behavior == Behavior::Allow)?,
        };
        if !matched {
            continue;
        }
        match rule.behavior {
            Behavior::Deny => return Ok(Some(Behavior::Deny)),
            Behavior::Ask => ask = true,
            Behavior::Allow => allow = true,
        }
    }
    Ok(if ask {
        Some(Behavior::Ask)
    } else if allow {
        Some(Behavior::Allow)
    } else {
        None
    })
}

/// Split statement and pipeline operators outside quoted strings. This is
/// deliberately not an AST substitute: PowerShell interpolation, script
/// blocks, call operators and redirections are marked opaque and require
/// an explicit one-time approval rather than inheriting a prefix allow.
/// The original parser handles these through AST security checks.
fn split_script(command: &str) -> (Vec<String>, bool) {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut comment = false;
    let mut escaped = false;
    let mut opaque = false;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if comment {
            if ch != '\n' && ch != '\r' {
                continue;
            }
            comment = false;
        }
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '`' {
            // Backtick escapes may hide operators or a newline.
            opaque = true;
            current.push(ch);
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            current.push(ch);
            if delimiter == '"' && ch == '$' && chars.peek() == Some(&'(') {
                // Double-quoted subexpressions execute code.
                opaque = true;
            }
            if ch == delimiter {
                if delimiter == '\'' && chars.peek() == Some(&'\'') {
                    // PowerShell single-quoted strings escape ' as ''.
                    current.push(chars.next().expect("peeked quote"));
                } else {
                    quote = None;
                }
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                current.push(ch);
            }
            '#' if current.chars().last().is_none_or(char::is_whitespace) => {
                comment = true;
            }
            ';' | '\n' | '\r' | '|' => {
                if ch == '|' && chars.peek() == Some(&'|') {
                    chars.next();
                }
                let part = current.trim();
                if !part.is_empty() {
                    parts.push(part.to_owned());
                }
                current.clear();
            }
            '&' if chars.peek() == Some(&'&') => {
                chars.next();
                let part = current.trim();
                if !part.is_empty() {
                    parts.push(part.to_owned());
                }
                current.clear();
            }
            // A single & is the call operator, not a statement separator.
            '&' | '{' | '}' | '(' | ')' | '>' | '<' => {
                opaque = true;
                current.push(ch);
            }
            '$' if chars.peek() == Some(&'(') => {
                opaque = true;
                current.push(ch);
            }
            _ => current.push(ch),
        }
    }
    if quote.is_some() || escaped {
        opaque = true;
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_owned());
    }
    (parts, opaque)
}

pub fn policy_for(rules: &[Rule], command: &str) -> Result<Option<Behavior>> {
    let (statements, opaque) = split_script(command);
    if statements.is_empty() {
        return Ok(Some(Behavior::Ask));
    }
    // Preserve exact, alias and wildcard behavior for ordinary commands.
    if statements.len() == 1 && !opaque {
        return policy_for_single(rules, command);
    }

    // A whole-script deny/ask still takes precedence, but a broad allow
    // cannot mask a denied command further down the script.
    let whole = policy_for_single(rules, command)?;
    if whole == Some(Behavior::Deny) {
        return Ok(whole);
    }
    let mut needs_approval = opaque || whole == Some(Behavior::Ask);
    let mut all_allowed = true;
    for statement in &statements {
        match policy_for_single(rules, statement)? {
            Some(Behavior::Deny) => return Ok(Some(Behavior::Deny)),
            Some(Behavior::Ask) => needs_approval = true,
            Some(Behavior::Allow) => {}
            None => all_allowed = false,
        }
    }
    if needs_approval {
        return Ok(Some(Behavior::Ask));
    }
    if all_allowed {
        return Ok(Some(Behavior::Allow));
    }
    // A user may have explicitly allowed this exact compound script.
    // Prefix/wildcard rules are insufficient for this exception.
    let exact_whole_allow = rules.iter().any(|rule| {
        rule.behavior == Behavior::Allow
            && rule.value.tool_name == "PowerShell"
            && rule.value.rule_content.as_deref() == Some(command.trim())
            && !has_wildcards(command)
            && extract_prefix(command).is_none()
    });
    if exact_whole_allow {
        Ok(Some(Behavior::Allow))
    } else {
        Ok(Some(Behavior::Ask))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission_rules::{compile_rules, RuleSetting};
    fn compiled(rules: &[(&str, &str)]) -> Vec<Rule> {
        compile_rules(
            &rules
                .iter()
                .map(|(behavior, rule)| RuleSetting {
                    behavior: (*behavior).into(),
                    rule: (*rule).into(),
                })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn raw_and_canonical_powershell_rules_do_not_see_bash() {
        let rules = compiled(&[
            ("allow", "Bash(Get-ChildItem:*)"),
            ("deny", "PowerShell(Remove-Item:*)"),
        ]);
        assert_eq!(
            policy_for(&rules, "rm secret").unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(&rules, "REMove-ItEM secret").unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(&rules, "rm\tsecret").unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(policy_for(&rules, "Get-ChildItem").unwrap(), None);
    }
    #[test]
    fn allow_alias_canonical_and_case_insensitivity() {
        let rules = compiled(&[("allow", "PowerShell(Get-ChildItem:*)")]);
        assert_eq!(policy_for(&rules, "ls").unwrap(), Some(Behavior::Allow));
        assert_eq!(
            policy_for(&rules, "GCI -Recurse").unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(&rules, "get-childitem\t-Recurse").unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(policy_for(&rules, "Get-ChildItemNot").unwrap(), None);
    }
    #[test]
    fn module_qualified_allow_is_not_cross_module_but_deny_is_fail_closed() {
        let rules = compiled(&[("allow", r"PowerShell(ModuleA\Get-Thing:*)")]);
        assert_eq!(policy_for(&rules, r"ModuleB\Get-Thing foo").unwrap(), None);
        let rules = compiled(&[("deny", r"PowerShell(ModuleA\Remove-Item:*)")]);
        assert_eq!(
            policy_for(&rules, "rm secret").unwrap(),
            Some(Behavior::Deny)
        );
        let rules = compiled(&[("deny", "PowerShell(rm *)")]);
        assert_eq!(
            policy_for(&rules, "Remove-Item secret").unwrap(),
            Some(Behavior::Deny)
        );
    }
    #[test]
    fn deny_wins_and_native_exe_aliases_are_not_inferred() {
        let rules = compiled(&[
            ("allow", "PowerShell(Remove-Item:*)"),
            ("ask", "PowerShell(Remove-Item:*)"),
            ("deny", "PowerShell(rm:*)"),
        ]);
        assert_eq!(
            policy_for(&rules, "Remove-Item secret").unwrap(),
            Some(Behavior::Deny)
        );
        let rules = compiled(&[("allow", "PowerShell(Get-ChildItem:*)")]);
        assert_eq!(policy_for(&rules, "sort test").unwrap(), None);
        assert_eq!(policy_for(&rules, r".\Get-ChildItem foo").unwrap(), None);
    }
    #[test]
    fn compound_scripts_cannot_bypass_later_denies_or_unknown_statements() {
        let rules = compiled(&[
            ("allow", "PowerShell(Get-ChildItem:*)"),
            ("allow", "PowerShell(Get-Content:*)"),
            ("deny", "PowerShell(Remove-Item:*)"),
        ]);
        for command in [
            "Get-ChildItem .; rm secret.txt",
            "Get-ChildItem . | Remove-Item secret.txt",
            "Get-ChildItem . && del secret.txt",
            "Get-ChildItem .\nRemove-Item secret.txt",
            "Get-ChildItem .; Get-Content foo; rm secret.txt",
        ] {
            assert_eq!(policy_for(&rules, command).unwrap(), Some(Behavior::Deny), "{command}");
        }
        assert_eq!(
            policy_for(&rules, "Get-ChildItem .; Invoke-Expression 'oops'").unwrap(),
            Some(Behavior::Ask)
        );
        assert_eq!(
            policy_for(&rules, "Get-ChildItem . | Get-Content file").unwrap(),
            Some(Behavior::Allow)
        );
    }

    #[test]
    fn quotes_comments_and_dynamic_script_blocks_do_not_inherit_prefix_allow() {
        let rules = compiled(&[
            ("allow", "PowerShell(Get-ChildItem:*)"),
            ("deny", "PowerShell(Remove-Item:*)"),
        ]);
        // Semicolons in quoted arguments and comments do not create commands.
        assert_eq!(
            policy_for(&rules, "Get-ChildItem 'a; rm b'").unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(&rules, "Get-ChildItem . # ; rm ignored").unwrap(),
            Some(Behavior::Allow)
        );
        // Scriptblocks, nested expressions and PowerShell escaping require
        // review even when the first cmdlet has a persisted allow rule.
        for command in [
            "Get-ChildItem . $(Remove-Item secret)",
            "Get-ChildItem . | %{ Remove-Item secret }",
            "Get-ChildItem . > output.txt",
            "Get-ChildItem . `; Remove-Item secret",
            "Get-ChildItem \"$(Remove-Item secret)\"",
            "Get-ChildItem 'unterminated",
        ] {
            assert_eq!(policy_for(&rules, command).unwrap(), Some(Behavior::Ask), "{command}");
        }
        assert_eq!(
            policy_for(&rules, "Get-ChildItem . # comment\nrm secret").unwrap(),
            Some(Behavior::Deny)
        );
    }

}
