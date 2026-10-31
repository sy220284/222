//! Rust exports derived from original:
//! src/components/memory/MemoryUpdateNotification.tsx
//!
//! Full equivalence: `getRelativeMemoryPath` — home-prefixed paths become
//! `~/...`, cwd-prefixed become `./...`, and the shorter of the two wins;
//! absolute path when neither applies.

/// `getRelativeMemoryPath`.
pub fn get_relative_memory_path(path: &str, home_dir: &str, cwd: &str) -> String {
    let rel_home = if path.starts_with(home_dir) {
        Some(format!("~{}", &path[home_dir.len()..]))
    } else {
        None
    };
    let rel_cwd = if path.starts_with(cwd) {
        Some(format!(
            "./{}",
            path.strip_prefix(cwd)
                .unwrap_or(path)
                .trim_start_matches('/')
        ))
    } else {
        None
    };
    match (rel_home, rel_cwd) {
        (Some(h), Some(c)) => {
            if h.len() <= c.len() {
                h
            } else {
                c
            }
        }
        (Some(h), None) => h,
        (None, Some(c)) => c,
        (None, None) => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorter_relative_wins() {
        assert_eq!(
            get_relative_memory_path("/h/m/a.md", "/h", "/h/w"),
            "~/m/a.md"
        );
        assert_eq!(
            get_relative_memory_path("/h/w/a.md", "/h", "/h/w"),
            "./a.md"
        );
        assert_eq!(get_relative_memory_path("/x/a.md", "/h", "/h/w"), "/x/a.md");
    }
}
