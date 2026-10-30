//! Rust exports derived from original: src/hooks/useMergedClients.ts
//!
//! `mergeClients` is the semantic core (a uniqBy-name concat); the React
//! memoization wrapper has no Rust host and the pure merge is the retained
//! behavior. MCPServerConnection carries a `name` in this crate's MCP types.

/// `uniqBy(list, 'name')` — first occurrence wins, order preserved.
pub fn uniq_by_name<T: Clone>(items: &[T], name_of: impl Fn(&T) -> String) -> Vec<T> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if seen.insert(name_of(item)) {
            out.push(item.clone());
        }
    }
    out
}

/// `mergeClients` — initial (configured) clients first, then live MCP
/// clients deduplicated by name.
pub fn merge_clients<T: Clone>(
    initial_clients: Option<&[T]>,
    mcp_clients: Option<&[T]>,
    name_of: impl Fn(&T) -> String,
) -> Vec<T> {
    match (initial_clients, mcp_clients) {
        (Some(initial), Some(mcp)) if !mcp.is_empty() => {
            let mut all = initial.to_vec();
            all.extend(mcp.iter().cloned());
            uniq_by_name(&all, name_of)
        }
        (Some(initial), _) => initial.to_vec(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mcp_clients_override_initials_by_name() {
        let initial = vec![("alpha", "cfg"), ("beta", "cfg")];
        let mcp = vec![("alpha", "live"), ("gamma", "live")];
        let merged =
            super::merge_clients(Some(&initial), Some(&mcp), |(name, _): &(&str, &str)| {
                (*name).to_owned()
            });
        let names: Vec<_> = merged.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn empty_live_queue_returns_initials() {
        let initial = vec![("alpha", "cfg")];
        let merged = super::merge_clients(
            Some(&initial),
            Some(&Vec::<(&str, &str)>::new()),
            |(name, _): &(&str, &str)| (*name).to_owned(),
        );
        assert_eq!(merged.len(), 1);
    }
}
