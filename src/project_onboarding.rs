//! Project onboarding state, adapted from the 2.1.88 projectOnboardingState.ts.
//! State is per canonical workspace and stored alongside the user's local
//! runtime configuration, not in the project being initialized.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub key: &'static str,
    pub text: &'static str,
    pub is_complete: bool,
    pub is_completable: bool,
    pub is_enabled: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    has_completed_project_onboarding: bool,
    #[serde(default)]
    project_onboarding_seen_count: u32,
}

pub struct Onboarding {
    root: PathBuf,
    state_path: PathBuf,
    state: State,
}

impl Onboarding {
    pub fn for_workspace(root: &Path) -> Result<Self> {
        let root = root.canonicalize().context("Workspace unavailable")?;
        let digest = Sha256::digest(root.to_string_lossy().as_bytes());
        let config_path = crate::config::Config::path()?;
        let base = config_path
            .parent()
            .context("Config location has no parent")?;
        let state_path = base.join(format!("project-onboarding-{:x}.json", digest));
        let state = match fs::read(&state_path) {
            Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| {
                format!("Invalid project onboarding state: {}", state_path.display())
            })?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            root,
            state_path,
            state,
        })
    }

    pub fn steps(&self) -> Result<Vec<Step>> {
        // Match the original: a bare workspace invites creation, an existing
        // workspace invites project instructions. Provider branding is removed.
        let mut entries = fs::read_dir(&self.root)?;
        let empty = entries.next().is_none();
        // The Agent loads all these provider-neutral project instruction
        // paths. An existing valid instruction file must satisfy onboarding,
        // or /init would prompt the user to create a redundant file.
        let has_instructions = [
            "AGENTS.md",
            "AGENTS.local.md",
            ".atlas/AGENTS.md",
            ".atlas/instructions.md",
            ".atlas/instructions.local.md",
        ]
            .iter()
            .any(|name| self.root.join(name).is_file());
        Ok(vec![
            Step {
                key: "workspace",
                text: "Ask the assistant to create an app or clone a repository",
                is_complete: false,
                is_completable: true,
                is_enabled: empty,
            },
            Step {
                key: "instructions",
                text: "Run /init to create project instructions in AGENTS.md",
                is_complete: has_instructions,
                is_completable: true,
                is_enabled: !empty,
            },
        ])
    }

    pub fn is_complete(&self) -> Result<bool> {
        Ok(self
            .steps()?
            .iter()
            .filter(|step| step.is_completable && step.is_enabled)
            .all(|step| step.is_complete))
    }

    pub fn should_show(&self) -> Result<bool> {
        if self.state.has_completed_project_onboarding
            || self.state.project_onboarding_seen_count >= 4
            || std::env::var_os("ATLAS_DEMO_MODE").is_some()
        {
            return Ok(false);
        }
        Ok(!self.is_complete()?)
    }

    pub fn current_suggestion(&self) -> Result<Option<&'static str>> {
        Ok(self
            .steps()?
            .into_iter()
            .find(|step| step.is_enabled && !step.is_complete)
            .map(|step| step.text))
    }

    pub fn mark_seen(&mut self) -> Result<()> {
        self.state.project_onboarding_seen_count =
            self.state.project_onboarding_seen_count.saturating_add(1);
        self.save()
    }

    pub fn maybe_mark_complete(&mut self) -> Result<()> {
        if !self.state.has_completed_project_onboarding && self.is_complete()? {
            self.state.has_completed_project_onboarding = true;
            self.save()?;
        }
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let parent = self
            .state_path
            .parent()
            .context("Onboarding state has no parent")?;
        fs::create_dir_all(parent)?;
        let temp = self
            .state_path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let serialized = serde_json::to_vec_pretty(&self.state)?;
        fs::write(&temp, serialized)?;
        if let Err(error) = fs::rename(&temp, &self.state_path) {
            let _ = fs::remove_file(&temp);
            return Err(error.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn empty_workspace_suggests_creating_a_project() {
        let work = tempfile::tempdir().unwrap();
        let settings = tempfile::tempdir().unwrap();
        let mut onboard = Onboarding {
            root: work.path().canonicalize().unwrap(),
            state_path: settings.path().join("state.json"),
            state: State::default(),
        };
        assert!(onboard.should_show().unwrap());
        assert_eq!(onboard.steps().unwrap()[0].key, "workspace");
        assert_eq!(
            onboard.current_suggestion().unwrap(),
            Some("Ask the assistant to create an app or clone a repository")
        );
        for _ in 0..4 {
            onboard.mark_seen().unwrap();
        }
        let saved: State = serde_json::from_slice(&fs::read(&onboard.state_path).unwrap()).unwrap();
        assert_eq!(saved.project_onboarding_seen_count, 4);
        assert!(!onboard.should_show().unwrap());
    }

    #[test]
    fn existing_project_tracks_instructions_without_touching_source_tree() {
        let work = tempfile::tempdir().unwrap();
        let settings = tempfile::tempdir().unwrap();
        fs::write(work.path().join("Cargo.toml"), "[package]").unwrap();
        let mut onboard = Onboarding {
            root: work.path().canonicalize().unwrap(),
            state_path: settings.path().join("state.json"),
            state: State::default(),
        };
        assert!(onboard.should_show().unwrap());
        assert_eq!(
            onboard.current_suggestion().unwrap(),
            Some("Run /init to create project instructions in AGENTS.md")
        );
        fs::write(work.path().join("AGENTS.md"), "Repository guidance").unwrap();
        onboard.maybe_mark_complete().unwrap();
        assert!(onboard.state.has_completed_project_onboarding);
        assert!(!onboard.should_show().unwrap());
        fs::remove_file(work.path().join("AGENTS.md")).unwrap();
        assert!(
            !onboard.should_show().unwrap(),
            "completed state is durable, as in the original"
        );
        assert!(!work.path().join(".atlas").exists());
    }
}
