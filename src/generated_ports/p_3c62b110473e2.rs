//! Rust exports derived from original: src/services/mcpServerApproval.tsx
//!
//! Pending project-server approval flow: collect project-scope server names
//! whose approval status is pending; nothing renders when the list is empty.
//! The dialog hosting (shared ink root, AppState + keybinding providers)
//! belongs to the ink runtime host; the server collection/decision core is
//! the portable part.

/// Project servers still awaiting approval.
pub fn pending_servers(project_servers: &[String], approved: &dyn Fn(&str) -> bool) -> Vec<String> {
    project_servers
        .iter()
        .filter(|s| !approved(s))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_pending_only() {
        let all = vec!["a".to_owned(), "b".to_owned()];
        let approved = |s: &str| s == "a";
        assert_eq!(pending_servers(&all, &approved), vec!["b".to_owned()]);
    }
}
