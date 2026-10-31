//! Local, provider-neutral port of 2.1.88 utils/crossProjectResume.ts.
//! Worktree membership is a path-boundary check: /repo/worktree2 must never
//! match /repo/worktree. Launch suggestions are displayed, never executed.
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResumeLocation {
    Current,
    SameRepositoryWorktree { project: PathBuf },
    AnotherProject { project: PathBuf, command: String },
}

pub fn check_cross_project_resume(
    current: &Path,
    project: Option<&Path>,
    show_all_projects: bool,
    worktree_paths: &[PathBuf],
    session_id: &str,
) -> ResumeLocation {
    let Some(project) = project else {
        return ResumeLocation::Current;
    };
    if !show_all_projects || project == current {
        return ResumeLocation::Current;
    }
    // Match src/utils/crossProjectResume.ts:54-56: equality or a descendant
    // separated by a path boundary, never a string-prefix sibling.
    if worktree_paths.iter().any(|wt| project == wt || project.starts_with(wt)) {
        return ResumeLocation::SameRepositoryWorktree {
            project: project.to_owned(),
        };
    }
    ResumeLocation::AnotherProject {
        project: project.to_owned(),
        command: resume_command(project, session_id),
    }
}

/// The original emits `cd <quoted dir> && claude --resume <id>`.
/// Rust replaces the branded binary with the local CLI and keeps explicit
/// quoting for paths and selectors, without executing any shell command.
pub fn resume_command(project: &Path, session_id: &str) -> String {
    format!(
        "atlas-agent --workspace {} chat --resume {}",
        crate::portable_platform::quote_argument(&project.to_string_lossy()),
        crate::portable_platform::quote_argument(session_id)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_path_hidden_projects_and_missing_project_are_current() {
        let base = Path::new("/work/current");
        assert_eq!(
            check_cross_project_resume(base, None, true, &[], "id"),
            ResumeLocation::Current
        );
        assert_eq!(
            check_cross_project_resume(base, Some(base), true, &[], "id"),
            ResumeLocation::Current
        );
        assert_eq!(
            check_cross_project_resume(base, Some(Path::new("/elsewhere")), false, &[], "id"),
            ResumeLocation::Current
        );
    }

    #[test]
    fn same_repository_worktree_requires_path_segment_boundary() {
        let roots = vec![PathBuf::from("/home/work/repo")];
        let actual = Path::new("/home/work/repo/feature");
        assert_eq!(
            check_cross_project_resume(
                Path::new("/home/work/main"),
                Some(actual),
                true,
                &roots,
                "id"
            ),
            ResumeLocation::SameRepositoryWorktree { project: actual.into() }
        );
        let adjacent = Path::new("/home/work/repository");
        assert!(matches!(
            check_cross_project_resume(
                Path::new("/home/work/main"),
                Some(adjacent),
                true,
                &roots,
                "id"
            ),
            ResumeLocation::AnotherProject { .. }
        ));
    }

    #[test]
    fn different_project_command_preserves_spaces_quotes_and_session() {
        let p = Path::new("/tmp/my project/it's here");
        let location = check_cross_project_resume(
            Path::new("/tmp/work"),
            Some(p),
            true,
            &[],
            "aa-bb"
        );
        assert_eq!(
            location,
            ResumeLocation::AnotherProject {
                project: p.into(),
                command: "atlas-agent --workspace '/tmp/my project/it'\"'\"'s here' chat --resume 'aa-bb'".to_owned(),
            }
        );
    }
}
