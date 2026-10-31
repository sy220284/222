//! Original src/utils/sandbox/sandbox-ui-utils.ts exact non-greedy tag filtering.
use regex::Regex;

pub fn remove_sandbox_violation_tags(text: &str) -> String {
    let pattern = Regex::new(r"(?s)<sandbox_violations>.*?</sandbox_violations>")
        .expect("reviewed constant regular expression");
    pattern.replace_all(text, "").into_owned()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_multiple_or_multiline_tags_without_touching_plain_text() {
        assert_eq!(
            remove_sandbox_violation_tags("a<sandbox_violations>x</sandbox_violations>b"),
            "ab"
        );
        assert_eq!(
            remove_sandbox_violation_tags("<sandbox_violations>\na\nb</sandbox_violations>"),
            ""
        );
        assert_eq!(
            remove_sandbox_violation_tags("<Sandbox_violations>x</Sandbox_violations>"),
            "<Sandbox_violations>x</Sandbox_violations>"
        );
    }
}
