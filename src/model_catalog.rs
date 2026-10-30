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
use std::{io, time::Duration};

#[derive(Debug, Clone, Subcommand)]
pub enum ModelAction {
    /// Interactive brand -> credential -> live model list -> verified selection.
    Setup {
        /// Optional brand ID; omit to choose interactively.
        brand: Option<String>,
    },
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
    Connect { brand: String },
    /// Check that an actual inference call succeeds for a particular model.
    Probe { brand: String, model: String },
    /// Verify the exact model with inference, then save it as the active model.
    Use { brand: String, model: String },
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
    let Some(name) = &profile.api_key_env else {
        return Ok(None);
    };
    if let Ok(key) = std::env::var(name) {
        if key.trim().is_empty() {
            bail!("API key environment variable {name} is empty")
        }
        return Ok(Some(key));
    }
    let path = key_store()?.path;
    if !path.exists() {
        bail!("Provider '{}' requires {name}; run 'atlas-agent models connect {}' or set the environment variable", profile.id, profile.id);
    }
    let raw = std::fs::read(&path).context("Read local model credentials")?;
    let stored: Value = serde_json::from_slice(&raw).context("Parse local model credentials")?;
    let key = stored
        .pointer(&format!("/model_api_keys/{}", profile.id))
        .and_then(Value::as_str)
        .filter(|key| !key.trim().is_empty())
        .with_context(|| {
            format!(
                "No API key for {}; run 'atlas-agent models connect {}'",
                profile.id, profile.id
            )
        })?;
    Ok(Some(key.to_owned()))
}

fn persist_key(brand: &str, key: &str) -> Result<()> {
    if key.trim().is_empty() {
        bail!("API key cannot be empty")
    }
    let store = key_store()?;
    let mut data = if store.path.exists() {
        serde_json::from_slice::<Value>(&std::fs::read(&store.path)?)
            .context("Parse existing model credentials")?
    } else {
        json!({"model_api_keys":{}})
    };
    if !data.is_object() {
        bail!("Invalid local model credentials")
    }
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
    if url.scheme() != "https" && !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"))
    {
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
    if let Some(levels) = value
        .pointer("/effort/supported_levels")
        .and_then(Value::as_array)
    {
        let levels = levels.iter().filter_map(Value::as_str).collect::<Vec<_>>();
        if !levels.is_empty() {
            tags.push(format!("思考:{}", levels.join("/")));
        }
    }
    if value
        .get("supports_function_calling")
        .and_then(Value::as_bool)
        == Some(true)
        || value.get("function_calling").and_then(Value::as_bool) == Some(true)
    {
        tags.push("工具调用:目录声明".into());
    }
    if value.get("supports_vision").and_then(Value::as_bool) == Some(true) {
        tags.push("视觉:目录声明".into());
    }
    tags
}

fn parsed_models(body: &Value, kind: &str, output: &mut Vec<ListedModel>) -> Result<()> {
    let items = body
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| body.get("models").and_then(Value::as_array))
        .context("Model catalog does not contain data or models array")?;
    for item in items {
        if item
            .get("visibility")
            .and_then(Value::as_str)
            .is_some_and(|value| value != "list")
        {
            continue;
        }
        if kind == "gemini" {
            let available = item
                .get("supportedGenerationMethods")
                .or_else(|| item.get("supportedActions"))
                .and_then(Value::as_array);
            if !available
                .is_some_and(|values| values.iter().any(|v| v.as_str() == Some("generateContent")))
            {
                continue;
            }
        }
        let id = item
            .get("id")
            .or_else(|| item.get("slug"))
            .or_else(|| item.get("name"))
            .and_then(Value::as_str);
        let Some(id) = id.filter(|id| !id.is_empty()) else {
            continue;
        };
        let id = if kind == "gemini" {
            id.strip_prefix("models/").unwrap_or(id)
        } else {
            id
        };
        let name = item
            .get("displayName")
            .or_else(|| item.get("display_name"))
            .or_else(|| item.get("name"))
            .and_then(Value::as_str)
            .unwrap_or(id);
        let mut tags = tags_from_metadata(item);
        if kind == "gemini" {
            tags.push("文本生成:目录声明".into());
            if item
                .pointer("/supportedGenerationMethods")
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("generateContent")))
            {
                tags.push("Gemini 原生接口".into());
            }
        }
        output.push(ListedModel {
            id: id.to_owned(),
            name: name.to_owned(),
            context_window: item
                .get("context_window")
                .or_else(|| item.get("context_length"))
                .or_else(|| item.get("inputTokenLimit"))
                .and_then(Value::as_u64),
            max_output_tokens: item
                .get("max_output_tokens")
                .or_else(|| item.get("outputTokenLimit"))
                .and_then(Value::as_u64),
            tags,
        });
    }
    Ok(())
}

/// Fetch all pages fresh. Never cache an account's authorized model catalog globally.
pub async fn list_models(profile: &ProviderProfile, key: &str) -> Result<Vec<ListedModel>> {
    if profile.kind == "chatgpt-plan" {
        let catalog = crate::chatgpt_plan::model_list().await?;
        let mut result = Vec::new();
        parsed_models(&catalog, "chatgpt-plan", &mut result)?;
        if result.is_empty() {
            bail!("ChatGPT account has no selectable models")
        }
        return Ok(result);
    }
    let mut models = Vec::new();
    let client = api_client()?;
    let mut page = None::<String>;
    for _ in 0..100 {
        let mut url = model_url(profile, "models")?;
        if profile.kind == "gemini" {
            url.query_pairs_mut().append_pair("pageSize", "1000");
            if let Some(value) = &page {
                url.query_pairs_mut().append_pair("pageToken", value);
            }
        } else if page.is_some() {
            bail!("Unexpected catalog pagination for {} API", profile.id);
        }
        let mut get = client.get(url);
        if profile.kind == "gemini" {
            get = get.header("x-goog-api-key", key);
        } else {
            get = get.bearer_auth(key);
        }
        let response = get.send().await.context("Model catalog request failed")?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "{} model catalog returned HTTP {}; check key permissions and API root URL",
                profile.id,
                status
            )
        }
        let bytes = response
            .bytes()
            .await
            .context("Read model catalog response")?;
        if bytes.len() > 4 * 1024 * 1024 {
            bail!("Model catalog page exceeds 4 MiB")
        }
        let body: Value = serde_json::from_slice(&bytes).context("Invalid model catalog JSON")?;
        parsed_models(&body, &profile.kind, &mut models)?;
        page = body
            .get("nextPageToken")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if page.is_none() {
            break;
        }
    }
    if page.is_some() {
        bail!("Model catalog contains more than 100 pages")
    }
    if models.is_empty() {
        bail!("Model catalog returned no selectable inference model IDs")
    }
    Ok(models)
}

/// A catalog listing is no substitute for an actual, billable inference request.
pub async fn probe_model(profile: &ProviderProfile, model: &str, key: &str) -> Result<()> {
    if model.trim().is_empty() || model.contains('/') && profile.kind != "gemini" {
        bail!("Invalid model ID");
    }
    let request = crate::protocol::ModelRequest {
        model: model.to_owned(),
        system: "Check model availability".into(),
        messages: vec![crate::protocol::Message::text("user", "Reply OK")],
        tools: Vec::new(),
    };
    if profile.kind == "chatgpt-plan" {
        crate::chatgpt_plan::complete(&request, &ModelTuning::default(), None).await?;
        return Ok(());
    }
    if profile.kind == "openai-responses"
        || (profile.id == "openai"
            && profile.base_url.trim_end_matches('/') == "https://api.openai.com/v1"
            && model.starts_with("gpt-6"))
    {
        crate::chatgpt_plan::complete_api(
            &request,
            &ModelTuning::default(),
            None,
            key,
            None,
            &profile.base_url,
        )
        .await?;
        return Ok(());
    }
    let (url, body) = if profile.kind == "gemini" {
        let id = model.strip_prefix("models/").unwrap_or(model);
        if id.contains('/') || id.contains(':') {
            bail!("Invalid Gemini model ID")
        }
        (
            model_url(profile, &format!("models/{id}:generateContent"))?,
            json!({"contents":[{"role":"user","parts":[{"text":"Reply OK"}]}],
                "generationConfig":{"maxOutputTokens":32}}),
        )
    } else {
        (
            model_url(profile, "chat/completions")?,
            json!({"model":model,
                "messages":[{"role":"user","content":"Reply OK"}],
                "max_tokens":32}),
        )
    };
    let mut post = api_client()?.post(url).json(&body);
    if profile.kind == "gemini" {
        post = post.header("x-goog-api-key", key);
    } else {
        post = post.bearer_auth(key);
    }
    let response = post
        .send()
        .await
        .context("Model callability probe failed")?;
    let status = response.status();
    if !status.is_success() {
        bail!(
            "Model '{}' for {} rejected inference with HTTP {}",
            model,
            profile.id,
            status
        )
    }
    let bytes = response
        .bytes()
        .await
        .context("Read model callability response")?;
    if bytes.len() > 4 * 1024 * 1024 {
        bail!("Model probe response exceeds 4 MiB")
    }
    let body: Value = serde_json::from_slice(&bytes)
        .context("Model probe returned non-JSON instead of an inference result")?;
    let has_result = if profile.kind == "gemini" {
        body.get("candidates")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    } else {
        body.get("choices")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    };
    if !has_result {
        bail!("Model probe returned HTTP 200 without a valid inference result");
    }
    Ok(())
}

fn profile_mut<'a>(config: &'a mut Config, brand: &str) -> Result<&'a mut ProviderProfile> {
    config
        .providers
        .iter_mut()
        .find(|p| p.id == brand)
        .with_context(|| format!("Unknown brand '{brand}'; run 'atlas-agent models'"))
}

fn read_stdin_line(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

async fn interactive_setup(config: &mut Config, requested: Option<&str>) -> Result<()> {
    let brand = if let Some(brand) = requested {
        brand.to_owned()
    } else {
        for (i, profile) in config.providers.iter().enumerate() {
            eprintln!("  {}. {} ({})", i + 1, profile.id, profile.model);
        }
        let selected = read_stdin_line("Choose model brand (number): ")?;
        let index = selected
            .parse::<usize>()
            .context("Brand must be a list number")?;
        config
            .providers
            .get(index.checked_sub(1).context("Brand index starts at 1")?)
            .context("Brand number is outside list")?
            .id
            .clone()
    };
    let profile = config.select(Some(&brand), None)?;
    let (models, key) = if profile.kind == "chatgpt-plan" {
        if !crate::chatgpt_plan::has_plan_session()? {
            crate::chatgpt_plan::run(&crate::chatgpt_plan::AccountAction::Login).await?;
        }
        (list_models(&profile, "").await?, None)
    } else {
        let key = if let Some(name) = &profile.api_key_env {
            match std::env::var(name) {
                Ok(key) if !key.trim().is_empty() => key,
                _ => {
                    eprintln!("For privacy, use an environment variable when possible; interactive input may echo.");
                    read_stdin_line(&format!("Enter API key for {brand}: "))?
                }
            }
        } else {
            read_stdin_line(&format!("Enter API key for {brand}: "))?
        };
        let models = list_models(&profile, &key).await?;
        (models, Some(key))
    };
    for (i, model) in models.iter().enumerate() {
        let context = model
            .context_window
            .map(|v| format!(" ctx={v}"))
            .unwrap_or_default();
        eprintln!(
            "  {}. {} — {}{} [{}]",
            i + 1,
            model.id,
            model.name,
            context,
            model.tags.join(", ")
        );
    }
    let selected = read_stdin_line("Choose an available model (number): ")?;
    let index = selected
        .parse::<usize>()
        .context("Model must be a list number")?;
    let model = models
        .get(index.checked_sub(1).context("Model index starts at 1")?)
        .context("Model number is outside list")?;
    probe_model(&profile, &model.id, key.as_deref().unwrap_or("")).await?;
    // Verify the selected model works before changing settings or persisting a new key.
    if let Some(key) = &key {
        if std::env::var(profile.api_key_env.as_deref().unwrap_or("")).is_err() {
            persist_key(&brand, key)?;
        }
    }
    {
        let selected = profile_mut(config, &brand)?;
        selected.model = model.id.clone();
        if brand == "openai"
            && selected.base_url.trim_end_matches('/') == "https://api.openai.com/v1"
            && model.id.starts_with("gpt-6")
        {
            selected.kind = "openai-responses".into();
        }
        // The live provider catalog owns the actual advertised context size.
        // Selecting a different model replaces stale automatic context metadata.
        selected.tuning.context_window = model
            .context_window
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n >= 1_024);
        if selected
            .tuning
            .max_output_tokens
            .zip(selected.tuning.context_window)
            .is_some_and(|(output, window)| output > window)
        {
            selected.tuning.max_output_tokens = None;
        }
    }
    config.default_provider = brand.clone();
    config.validate()?;
    config.save()?;
    println!("Model verified and selected: {brand} / {}", model.id);
    Ok(())
}

pub async fn run(action: &ModelAction, config: &mut Config) -> Result<()> {
    match action {
        ModelAction::Setup { brand } => interactive_setup(config, brand.as_deref()).await?,
        ModelAction::Brands => {
            for p in &config.providers {
                println!("{}: {} | {} | {:?}", p.id, p.model, p.base_url, p.tuning);
            }
        }
        ModelAction::Discover { brand, key_stdin } => {
            let profile = config.select(Some(brand), None)?;
            let key = if profile.kind == "chatgpt-plan" {
                if *key_stdin {
                    bail!("ChatGPT plan uses 'chatgpt login' and does not accept an API key")
                }
                String::new()
            } else if *key_stdin {
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
            if profile.kind == "chatgpt-plan" {
                bail!("ChatGPT plan uses 'atlas-agent chatgpt login'");
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
            let key = if profile.kind == "chatgpt-plan" {
                String::new()
            } else {
                load_api_key(&profile)?.context("Model probe requires API credentials")?
            };
            probe_model(&profile, model, &key).await?;
            println!("Verified callable: {brand} / {model}");
            if matches!(action, ModelAction::Use { .. }) {
                let catalog = list_models(&profile, &key).await;
                let context = match catalog {
                    Ok(available) => available
                        .iter()
                        .find(|item| item.id == *model)
                        .and_then(|item| item.context_window)
                        .and_then(|value| u32::try_from(value).ok())
                        .filter(|value| *value >= 1_024),
                    Err(error) => {
                        eprintln!("Model was verified, but context metadata could not be retrieved: {error:#}");
                        None
                    }
                };
                let selected = profile_mut(config, brand)?;
                selected.model = model.clone();
                if brand == "openai"
                    && selected.base_url.trim_end_matches('/') == "https://api.openai.com/v1"
                    && model.starts_with("gpt-6")
                {
                    selected.kind = "openai-responses".into();
                }
                if selected.model != profile.model {
                    selected.tuning.context_window = context;
                    if selected
                        .tuning
                        .max_output_tokens
                        .zip(selected.tuning.context_window)
                        .is_some_and(|(out, window)| out > window)
                    {
                        selected.tuning.max_output_tokens = None;
                    }
                }
                config.default_provider = brand.clone();
                config.validate()?;
                config.save()?;
                println!("Selected default model: {brand} / {model}");
            }
        }
        ModelAction::Tune {
            brand,
            temperature,
            top_p,
            max_output_tokens,
            context_window,
            reasoning_effort,
            reset,
        } => {
            let p = profile_mut(config, brand)?;
            if *reset {
                p.tuning = ModelTuning::default();
            }
            if let Some(v) = temperature {
                p.tuning.temperature = Some(*v);
            }
            if let Some(v) = top_p {
                p.tuning.top_p = Some(*v);
            }
            if let Some(v) = max_output_tokens {
                p.tuning.max_output_tokens = Some(*v);
            }
            if let Some(v) = context_window {
                p.tuning.context_window = Some(*v);
            }
            if let Some(v) = reasoning_effort {
                p.tuning.reasoning_effort = Some(v.clone());
            }
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
            vec![
                "输入:text",
                "输入:image",
                "思考:low/high",
                "工具调用:目录声明"
            ]
        );
        assert!(tags_from_metadata(&json!({"id":"x"})).is_empty());
    }

    #[tokio::test]
    async fn catalog_and_probe_use_brand_specific_auth_and_native_routes() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for brand in ["deepseek", "gemini"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let brand_name = brand.to_owned();
            let worker = tokio::spawn(async move {
                for request_index in 0..if brand_name == "gemini" { 3 } else { 2 } {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut received = Vec::new();
                    let mut buffer = [0u8; 4096];
                    let end = loop {
                        let n = socket.read(&mut buffer).await.unwrap();
                        assert!(n > 0);
                        received.extend_from_slice(&buffer[..n]);
                        if let Some(idx) = received.windows(4).position(|v| v == b"\r\n\r\n") {
                            break idx + 4;
                        }
                    };
                    let headers = String::from_utf8_lossy(&received[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    while received.len() < end + length {
                        let n = socket.read(&mut buffer).await.unwrap();
                        assert!(n > 0);
                        received.extend_from_slice(&buffer[..n]);
                    }
                    if brand_name == "gemini" {
                        assert!(headers.contains("x-goog-api-key: test-key"));
                        assert!(!headers.contains("authorization: bearer"));
                    } else {
                        assert!(headers.contains("authorization: bearer test-key"));
                    }
                    let (reply, expected) = if brand_name == "gemini" {
                        match request_index {
                            0 => (
                                json!({"models":[{"name":"models/gemini-test",
                                "displayName":"Gemini Test", "inputTokenLimit":1000,
                                "outputTokenLimit":150,"supportedGenerationMethods":["generateContent"]},
                                {"name":"models/embedding-test",
                                "supportedGenerationMethods":["embedContent"]}],
                                "nextPageToken":"page-2"}),
                                "GET /models?pageSize=1000",
                            ),
                            1 => (
                                json!({"models":[{"name":"models/gemini-test-2",
                                "supportedGenerationMethods":["generateContent"]}]}),
                                "GET /models?pageSize=1000&pageToken=page-2",
                            ),
                            _ => (
                                json!({"candidates":[{"content":{"parts":[{"text":"OK"}]}}]}),
                                "POST /models/gemini-test:generateContent",
                            ),
                        }
                    } else if request_index == 0 {
                        (
                            json!({"data":[{"id":"deepseek-v4-pro","name":"DeepSeek Pro",
                            "context_window":1048576,"max_output_tokens":1024,
                            "input_modalities":["text"],"effort":{"supported_levels":["low","high"]}}]}),
                            "GET /models",
                        )
                    } else {
                        (
                            json!({"choices":[{"message":{"role":"assistant","content":"OK"}}]}),
                            "POST /chat/completions",
                        )
                    };
                    assert!(
                        headers.contains(&expected.to_ascii_lowercase()),
                        "{brand_name}: {headers}"
                    );
                    let payload = reply.to_string();
                    socket.write_all(format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        payload.len(), payload,
                    ).as_bytes()).await.unwrap();
                }
            });
            let mut profile = Config::default().select(Some(brand), None).unwrap();
            profile.base_url = format!("http://{address}");
            let discovered = list_models(&profile, "test-key").await.unwrap();
            if brand == "gemini" {
                assert_eq!(
                    discovered.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
                    vec!["gemini-test", "gemini-test-2"]
                );
                assert_eq!(discovered[0].context_window, Some(1000));
                assert_eq!(discovered[0].max_output_tokens, Some(150));
                probe_model(&profile, "gemini-test", "test-key")
                    .await
                    .unwrap();
            } else {
                assert_eq!(discovered.len(), 1);
                assert_eq!(discovered[0].tags, vec!["输入:text", "思考:low/high"]);
                assert_eq!(discovered[0].context_window, Some(1048576));
                probe_model(&profile, "deepseek-v4-pro", "test-key")
                    .await
                    .unwrap();
            }
            worker.await.unwrap();
        }
    }

    #[test]
    fn tuning_rejects_invalid_values() {
        assert!(ModelTuning {
            temperature: Some(2.1),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(ModelTuning {
            top_p: Some(0.0),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(ModelTuning {
            max_output_tokens: Some(900),
            context_window: Some(800),
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
