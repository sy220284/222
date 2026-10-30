//! Provider-owned model discovery, explicit callability probes and local credentials.
//! A GET /models result is a catalog, never proof of inference entitlement.
use crate::{
    config::{Config, ModelTuning, ProviderProfile},
    plain_text_storage::PlainTextStorage,
    secure_fallback::SecureStorage,
};
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io, time::Duration};

#[derive(Debug, Clone, Subcommand)]
pub enum ModelAction {
    /// List built-in brands, recommended versions, and current user settings.
    Brands,
    /// Query the selected brand's /models with the current API key.
    Discover {
        brand: String,
        /// Read an API key from stdin for this discovery only; do not persist it.
        #[arg(long)]
        key_stdin: bool,
    },
    /// Read an API key from stdin, verify it against /models, and save it locally.
    Connect {
        brand: String,
    },
    /// Check that an actual inference call succeeds for a particular model.
    Probe {
        brand: String,
        model: String,
    },
    /// Verify the exact model with inference, then save it as the active model.
    Use {
        brand: String,
        model: String,
    },
    /// Change provider sampling limits; omitted values preserve previous settings.
    Tune {
        brand: String,
        #[arg(long)]
        temperature: Option<f64>,
        #[arg(long)]
        top_p: Option<f64>,
        #[arg(long)]
        max_output_tokens: Option<u32>,
        #[arg(long)]
        context_window: Option<u32>,
        #[arg(long)]
        reasoning_effort: Option<String>,
        #[arg(long)]
        reset: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListedModel {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    pub tags: Vec<String>,
}

fn key_store() -> Result<PlainTextStorage> {
    Ok(PlainTextStorage::new(
        Config::path()?.with_file_name(".model-api-keys.json"),
    ))
}

/// Environment variables win over saved credentials; no keys are put in config.json.
pub fn load_api_key(profile: &ProviderProfile) -> Result<Option<String>> {
    let Some(name) = &profile.api_key_env else { return Ok(None) };
    if let Ok(key) = std::env::var(name) {
        if key.trim().is_empty() { bail!("API key environment variable {name} is empty") }
        return Ok(Some(key));
    }
    let path = key_store()?.path;
    if !path.exists() {
        bail!("Provider '{}' requires {name}; run 'atlas-agent models connect {}' or set the environment variable", profile.id, profile.id);
    }
    let raw = std::fs::read(&path).context("Read local model credentials")?;
    let stored: Value = serde_json::from_slice(&raw).context("Parse local model credentials")?;
    let key = stored.pointer(&format!("/model_api_keys/{}", profile.id))
        .and_then(Value::as_str)
        .filter(|key| !key.trim().is_empty())
        .with_context(|| format!("No API key for {}; run 'atlas-agent models connect {}'", profile.id, profile.id))?;
    Ok(Some(key.to_owned()))
}

fn persist_key(brand: &str, key: &str) -> Result<()> {
    if key.trim().is_empty() { bail!("API key cannot be empty") }
    let store = key_store()?;
    let mut data = if store.path.exists() {
        serde_json::from_slice::<Value>(&std::fs::read(&store.path)?)
            .context("Parse existing model credentials")?
    } else {
        json!({"model_api_keys":{}})
    };
    if !data.is_object() { bail!("Invalid local model credentials") }
    if !data["model_api_keys"].is_object() {
        data["model_api_keys"] = json!({});
    }
    data["model_api_keys"][brand] = json!(key);
    if !store.update(data).success {
        bail!("Failed to securely save model credentials")
    }
    Ok(())
}

fn api_client() -> Result<Client> {
    Ok(crate::tls_ca::client_builder()?
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

fn model_url(profile: &ProviderProfile, route: &str) -> Result<Url> {
    let base = profile.base_url.trim_end_matches('/');
    let url = Url::parse(&format!("{base}/{route}"))?;
    if url.scheme() != "https" && !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1")) {
        bail!("Model API requests require HTTPS except loopback test endpoints");
    }
    Ok(url)
}

fn tags_from_metadata(value: &Value) -> Vec<String> {
    let mut tags = Vec::new();
    for (field, label) in [("input_modalities", "输入"), ("output_modalities", "输出")] {
        if let Some(items) = value.get(field).and_then(Value::as_array) {
            for item in items {
                if let Some(name) = item.as_str() {
                    tags.push(format!("{label}:{name}"));
                }
            }
        }
    }
    if let Some(levels) = value.pointer("/effort/supported_levels").and_then(Value::as_array) {
        let levels = levels.iter().filter_map(Value::as_str).collect::<Vec<_>>();
        if !levels.is_empty() {
            tags.push(format!("思考:{}", levels.join("/")));
        }
    }
    if value.get("supports_function_calling").and_then(Value::as_bool) == Some(true)
        || value.get("function_calling").and_then(Value::as_bool) == Some(true)
    {
        tags.push("工具调用:目录声明".into());
    }
    if value.get("supports_vision").and_then(Value::as_bool) == Some(true) {
        tags.push("视觉:目录声明".into());
    }
    tags
}

pub async fn list_models(profile: &ProviderProfile, key: &str) -> Result<Vec<ListedModel>> {
    let url = model_url(profile, "models")?;
    let response = api_client()?.get(url).bearer_auth(key).send().await
        .context("Model catalog request failed")?;
    let status = response.status();
    if !status.is_success() {
        bail!("{} model catalog returned HTTP {}; use the provider's documented model ID if it does not support /models", profile.id, status)
    }
    let bytes = response.bytes().await.context("Read model catalog response")?;
    if bytes.len() > 2 * 1024 * 1024 {
        bail!("Model catalog response exceeds 2 MiB");
    }
    let body: Value = serde_json::from_slice(&bytes).context("Invalid model catalog JSON")?;
    let items = body.get("data").and_then(Value::as_array)
        .or_else(|| body.get("models").and_then(Value::as_array))
        .context("Model catalog does not contain data or models array")?;
    let mut models = Vec::new();
    for item in items {
        let id = item.get("id").or_else(|| item.get("slug")).and_then(Value::as_str);
        let Some(id) = id.filter(|v| !v.is_empty()) else { continue };
        if item.get("visibility").and_then(Value::as_str).is_some_and(|v| v != "list") {
            continue;
        }
        models.push(ListedModel {
            id: id.to_owned(),
            name: item.get("name").or_else(|| item.get("display_name"))
                .and_then(Value::as_str).unwrap_or(id).to_owned(),
            context_window: item.get("context_window").or_else(|| item.get("context_length"))
                .and_then(Value::as_u64),
            max_output_tokens: item.get("max_output_tokens").and_then(Value::as_u64),
            tags: tags_from_metadata(item),
        });
    }
    if models.is_empty() { bail!("Model catalog returned no selectable model IDs") }
    Ok(models)
}

pub async fn probe_model(profile: &ProviderProfile, model: &str, key: &str) -> Result<()> {
    if model.trim().is_empty() { bail!("Model ID cannot be empty") }
    let url = model_url(profile, "chat/completions")?;
    // A successful request confirms availability for this key and model. Small
    // output limit reduces billable tokens but does not imply the request is free.
    let mut body = json!({
        "model":model,
        "messages":[{"role":"user","content":"Reply OK"}],
        "max_tokens":32
    });
    if profile.id == "deepseek" {
        body["reasoning_effort"] = json!("none");
    }
    let response = api_client()?.post(url).bearer_auth(key).json(&body)
        .send().await.context("Model callability probe failed")?;
    let status = response.status();
    if !status.is_success() {
        bail!("Model '{}' for {} rejected inference with HTTP {}", model, profile.id, status)
    }
    Ok(())
}

fn profile_mut<'a>(config: &'a mut Config, brand: &str) -> Result<&'a mut ProviderProfile> {
    config.providers.iter_mut().find(|p| p.id == brand)
        .with_context(|| format!("Unknown brand '{brand}'; run 'atlas-agent models'"))
}

pub async fn run(action: &ModelAction, config: &mut Config) -> Result<()> {
    match action {
        ModelAction::Brands => {
            for p in &config.providers {
                println!("{}: {} | {} | {:?}", p.id, p.model, p.base_url, p.tuning);
            }
        }
        ModelAction::Discover { brand, key_stdin } => {
            let profile = config.select(Some(brand), None)?;
            let key = if *key_stdin {
                let mut line = String::new();
                io::stdin().read_line(&mut line)?;
                line.trim_end().to_owned()
            } else {
                load_api_key(&profile)?.context("Model discovery needs an API key")?
            };
            for model in list_models(&profile, &key).await? {
                println!("{}", serde_json::to_string(&model)?);
            }
        }
        ModelAction::Connect { brand } => {
            let profile = config.select(Some(brand), None)?;
            if profile.kind != "openai-compatible" {
                bail!("Use API key setup for OpenAI-compatible brands only");
            }
            eprintln!("Enter the API key for {brand} on stdin (the key will be saved in an owner-only local file):");
            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim_end();
            // Only save credentials after the service accepts them.
            let models = list_models(&profile, key).await?;
            persist_key(brand, key)?;
            eprintln!("Authenticated {brand}; {} model IDs returned", models.len());
            for model in models {
                println!("{}", serde_json::to_string(&model)?);
            }
            eprintln!("Choose one with: atlas-agent models use {brand} MODEL_ID");
        }
        ModelAction::Probe { brand, model } | ModelAction::Use { brand, model } => {
            let profile = config.select(Some(brand), None)?;
            let key = load_api_key(&profile)?.context("Model probe requires API credentials")?;
            probe_model(&profile, model, &key).await?;
            println!("Verified callable: {brand} / {model}");
            if matches!(action, ModelAction::Use { .. }) {
                profile_mut(config, brand)?.model = model.clone();
                config.default_provider = brand.clone();
                config.validate()?;
                config.save()?;
                println!("Selected default model: {brand} / {model}");
            }
        }
        ModelAction::Tune {
            brand, temperature, top_p, max_output_tokens,
            context_window, reasoning_effort, reset,
        } => {
            let p = profile_mut(config, brand)?;
            if *reset { p.tuning = ModelTuning::default(); }
            if let Some(v) = temperature { p.tuning.temperature = Some(*v); }
            if let Some(v) = top_p { p.tuning.top_p = Some(*v); }
            if let Some(v) = max_output_tokens { p.tuning.max_output_tokens = Some(*v); }
            if let Some(v) = context_window { p.tuning.context_window = Some(*v); }
            if let Some(v) = reasoning_effort { p.tuning.reasoning_effort = Some(v.clone()); }
            config.validate()?;
            config.save()?;
            println!("Updated {brand} model parameters");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abilities_only_come_from_explicit_server_metadata() {
        let model = json!({
            "input_modalities":["text","image"],
            "effort":{"supported_levels":["low","high"]},
            "supports_function_calling":true
        });
        assert_eq!(
            tags_from_metadata(&model),
            vec!["输入:text", "输入:image", "思考:low/high", "工具调用:目录声明"]
        );
        assert!(tags_from_metadata(&json!({"id":"x"})).is_empty());
    }
    #[test]
    fn tuning_rejects_invalid_values() {
        assert!(ModelTuning { temperature: Some(2.1), ..Default::default() }.validate().is_err());
        assert!(ModelTuning { top_p: Some(0.0), ..Default::default() }.validate().is_err());
        assert!(ModelTuning { max_output_tokens: Some(900), context_window: Some(800),
            ..Default::default() }.validate().is_err());
    }
}
