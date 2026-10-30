//! Provider-neutral implementation of `src/commands/advisor.ts`.
//! Model selection is resolved against configured provider profiles so a
//! model that cannot actually be addressed is not silently accepted.
use crate::config::{Config, ProviderProfile};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvisorChange {
    Unchanged(String),
    Set {
        provider_id: String,
        message: String,
    },
    Unset(String),
}

fn find_profile<'a>(config: &'a Config, name: &str) -> Option<&'a ProviderProfile> {
    // The UI accepts either a provider ID or an exact configured model name.
    // Only an unambiguous model name can select a provider; IDs are unique.
    if let Some(profile) = config
        .providers
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(name))
    {
        return Some(profile);
    }
    let mut found = config
        .providers
        .iter()
        .filter(|p| p.model.eq_ignore_ascii_case(name));
    let first = found.next()?;
    if found.next().is_some() {
        None
    } else {
        Some(first)
    }
}

/// Source-level `call`: resolves command state and messages with no side effects.
/// Mutation and persistence are performed only after model validation.
pub fn resolve(config: &Config, active: &ProviderProfile, args: &str) -> AdvisorChange {
    let arg = args.trim().to_ascii_lowercase();
    if arg.is_empty() {
        return AdvisorChange::Unchanged(match config.advisor_model.as_deref() {
            None => "Advisor: not set\nUse \"/advisor <model>\" to enable.".into(),
            Some(id) if config.providers.iter().any(|p| p.id == id) => format!(
                "Advisor: {id}\nUse \"/advisor unset\" to disable or \"/advisor <model>\" to change."
            ),
            Some(id) => format!("Advisor: {id} (inactive)\nThe configured advisor provider is unavailable."),
        });
    }
    if arg == "unset" || arg == "off" {
        return AdvisorChange::Unset(match config.advisor_model.as_deref() {
            Some(id) => format!("Advisor disabled (was {id})."),
            None => "Advisor already unset.".into(),
        });
    }
    let Some(selected) = find_profile(config, &arg) else {
        return AdvisorChange::Unchanged(format!(
            "Unknown advisor model: {arg}. Configure a provider or select a registered model."
        ));
    };
    let message = if selected.id == active.id {
        format!("Advisor set to {}.", selected.id)
    } else {
        format!("Advisor set to {} ({}).", selected.id, selected.model)
    };
    AdvisorChange::Set {
        provider_id: selected.id.clone(),
        message,
    }
}

/// Persist user-level advisor choice using the same atomic config writer as
/// model/provider settings. If the write fails, the live state is unchanged.
pub fn execute(config: &mut Config, active: &ProviderProfile, args: &str) -> Result<String> {
    let result = resolve(config, active, args);
    match result {
        AdvisorChange::Unchanged(message) => Ok(message),
        AdvisorChange::Set {
            provider_id,
            message,
        } => {
            if config.advisor_model.as_deref() != Some(provider_id.as_str()) {
                let mut next = config.clone();
                next.advisor_model = Some(provider_id);
                next.save()?;
                *config = next;
            }
            Ok(message)
        }
        AdvisorChange::Unset(message) => {
            if config.advisor_model.is_some() {
                let mut next = config.clone();
                next.advisor_model = None;
                next.save()?;
                *config = next;
            }
            Ok(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn show_set_disable_and_validate() {
        let mut config = Config::default();
        let active = config.select(None, None).unwrap();
        assert!(
            matches!(resolve(&config, &active, "  "), AdvisorChange::Unchanged(s) if s.contains("not set"))
        );
        assert_eq!(
            resolve(&config, &active, "bad"),
            AdvisorChange::Unchanged(
                "Unknown advisor model: bad. Configure a provider or select a registered model."
                    .into()
            )
        );
        assert!(
            matches!(resolve(&config, &active, "  OPENAI "), AdvisorChange::Set { provider_id, .. } if provider_id == "openai")
        );
        assert!(
            matches!(resolve(&config, &active, "  CHATGPT "), AdvisorChange::Set { provider_id, .. } if provider_id == "chatgpt")
        );
        // Both API-key OpenAI and ChatGPT-plan profiles advertise the same
        // model: users must select the provider explicitly to avoid ambiguity.
        assert!(matches!(
            resolve(&config, &active, "gpt-6.1-sol"),
            AdvisorChange::Unchanged(_)
        ));
        config.advisor_model = Some("openai".into());
        assert!(
            matches!(resolve(&config, &active, "off"), AdvisorChange::Unset(s) if s.contains("openai"))
        );
        assert!(
            matches!(resolve(&config, &active, ""), AdvisorChange::Unchanged(s) if s.contains("Advisor: openai"))
        );
        config.providers.push(ProviderProfile {
            id: "duplicate".into(),
            kind: "openai-compatible".into(),
            base_url: "https://localhost".into(),
            model: "gpt-6.1-sol".into(),
            api_key_env: None,
            stream: false,
            tuning: Default::default(),
        });
        assert!(matches!(
            resolve(&config, &active, "gpt-6.1-sol"),
            AdvisorChange::Unchanged(_)
        ));
    }
}
