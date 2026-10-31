//! Rust exports derived from original: src/hooks/useMergedCommands.ts
//!
//! `useMergedCommands` semantic core: MCP commands (when present) are merged
//! after initial commands and deduplicated by name, first occurrence wins.
//! Commands are identified by name via this crate's command surface.

/// `uniqBy([...initialCommands, ...mcpCommands], 'name')` — only when MCP
/// commands exist; otherwise the initial list is returned as-is.
pub fn merge_commands<T: Clone>(
    initial_commands: &[T],
    mcp_commands: &[T],
    name_of: impl Fn(&T) -> String,
) -> Vec<T> {
    if !mcp_commands.is_empty() {
        let mut all = initial_commands.to_vec();
        all.extend(mcp_commands.iter().cloned());
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out = Vec::with_capacity(all.len());
        for command in all {
            if seen.insert(name_of(&command)) {
                out.push(command);
            }
        }
        out
    } else {
        initial_commands.to_vec()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mcp_commands_dedupe_by_name_after_initials() {
        let initial = vec!["help", "config"];
        let mcp = vec!["config", "server1:tool"];
        let merged = super::merge_commands(&initial, &mcp, |c| (*c).to_owned());
        assert_eq!(merged, vec!["help", "config", "server1:tool"]);
    }

    #[test]
    fn no_mcp_commands_keeps_initial_list_intact() {
        let initial = vec!["help"];
        let merged = super::merge_commands(&initial, &Vec::<&str>::new(), |c| (*c).to_owned());
        assert_eq!(merged, vec!["help"]);
    }
}
