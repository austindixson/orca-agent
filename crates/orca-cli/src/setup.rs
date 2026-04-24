//! Interactive `orca setup` wizard (Hermes-style).

use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use anyhow::Context;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use serde_json::Value;

use crate::config::{
    bridge_token_store_keyring, telegram_token_from_keyring, telegram_token_store_keyring,
    OrcaConfig,
};

#[derive(Clone, Copy)]
enum SetupMode {
    Quick,
    Full,
    TelegramOnly,
}

#[derive(Clone, Copy)]
enum ProviderPreset {
    OpenRouter,
    OpenAi,
    Anthropic,
    Xai,
    Zai,
    Mistral,
    GithubCopilot,
    GoogleVertex,
    AzureOpenAi,
    Ollama,
    HermesGateway,
    Custom,
}

impl ProviderPreset {
    fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Xai => "xAI (Grok)",
            Self::Zai => "Z.AI (GLM)",
            Self::Mistral => "Mistral",
            Self::GithubCopilot => "GitHub Copilot",
            Self::GoogleVertex => "Google Vertex (OpenAI-compatible)",
            Self::AzureOpenAi => "Azure OpenAI",
            Self::Ollama => "Ollama (local)",
            Self::HermesGateway => "Hermes Gateway",
            Self::Custom => "Custom OpenAI-compatible endpoint",
        }
    }

    fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Xai => "https://api.x.ai/v1",
            Self::Zai => "https://api.z.ai/api/coding/paas/v4",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::GithubCopilot => "https://api.githubcopilot.com",
            Self::GoogleVertex => "https://aiplatform.googleapis.com/v1beta1/projects/PROJECT/locations/global/endpoints/openapi",
            Self::AzureOpenAi => "https://YOUR-RESOURCE.openai.azure.com/openai/deployments/YOUR-DEPLOYMENT",
            Self::Ollama => "http://localhost:11434/v1",
            Self::HermesGateway => "http://127.0.0.1:8642/v1",
            Self::Custom => "http://localhost:11434/v1",
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::OpenRouter => "openai/gpt-4o-mini",
            Self::OpenAi => "gpt-4o-mini",
            Self::Anthropic => "claude-3-5-sonnet-latest",
            Self::Xai => "grok-4",
            Self::Zai => "GLM-4.7",
            Self::Mistral => "mistral-medium-latest",
            Self::GithubCopilot => "gpt-4.1",
            Self::GoogleVertex => "google/gemini-2.5-pro",
            Self::AzureOpenAi => "gpt-4.1",
            Self::Ollama => "qwen2.5-coder:14b",
            Self::HermesGateway => "gpt-5.4-mini",
            Self::Custom => "your-model-id",
        }
    }

    fn key_prompt(self) -> &'static str {
        match self {
            Self::OpenRouter => {
                "OpenRouter API key (optional now; you can set OPENROUTER_API_KEY later)"
            }
            Self::OpenAi => "OpenAI API key (optional now; you can set OPENAI_API_KEY later)",
            Self::Anthropic => {
                "Anthropic API key (optional now; you can set ANTHROPIC_API_KEY later)"
            }
            Self::Xai => "xAI API key (optional now; you can set XAI_API_KEY later)",
            Self::Zai => {
                "Z.AI API key (optional now; you can set ZAI_API_KEY or GLM_API_KEY later)"
            }
            Self::Mistral => "Mistral API key (optional now; you can set MISTRAL_API_KEY later)",
            Self::GithubCopilot => "GitHub token (optional now; you can set GITHUB_TOKEN later)",
            Self::GoogleVertex => "Google Vertex bearer token/API key (optional now)",
            Self::AzureOpenAi => "Azure OpenAI API key (optional now)",
            Self::Ollama => "API key (optional; usually empty for local Ollama)",
            Self::HermesGateway => "Hermes API key (optional; often empty for local gateway)",
            Self::Custom => "API key (optional; leave empty if your endpoint does not require one)",
        }
    }
}

fn first_nonempty_env(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Non-interactive: merge `PORT`, `WORKSPACE_ROOT`, provider API-key envs, `ORCA_MODEL`, `ORCA_LLM_BASE_URL` into config and save.
pub fn cmd_setup_defaults() -> anyhow::Result<()> {
    let mut cfg = OrcaConfig::load().unwrap_or_default();

    if let Ok(p) = std::env::var("PORT") {
        if let Ok(port) = p.trim().parse::<u16>() {
            cfg.server.port = port;
        }
    }
    if let Ok(w) = std::env::var("WORKSPACE_ROOT") {
        if !w.trim().is_empty() {
            cfg.server.workspace = Some(PathBuf::from(w.trim()));
        }
    }
    if let Some(k) = first_nonempty_env(&[
        "ORCA_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "XAI_API_KEY",
        "ZAI_API_KEY",
        "GLM_API_KEY",
        "MISTRAL_API_KEY",
        "GITHUB_TOKEN",
    ]) {
        cfg.llm.api_key = Some(k);
    }
    if let Ok(u) = std::env::var("ORCA_LLM_BASE_URL") {
        if !u.trim().is_empty() {
            cfg.llm.base_url = Some(u);
        }
    }
    if let Ok(m) = std::env::var("ORCA_MODEL") {
        if !m.trim().is_empty() {
            cfg.llm.model = Some(m);
        }
    }

    if cfg.bridge.token.is_none() {
        let tok = uuid::Uuid::new_v4().to_string();
        cfg.bridge.token = Some(tok.clone());
        let _ = bridge_token_store_keyring(&tok);
    }

    cfg.save()?;
    println!(
        "Wrote {} (from env defaults)",
        OrcaConfig::path()?.display()
    );
    Ok(())
}

fn choose_setup_mode(theme: &ColorfulTheme, is_existing: bool) -> anyhow::Result<SetupMode> {
    let (prompt, items, default_idx) = if is_existing {
        (
            "Setup mode",
            vec![
                "Quick setup (recommended)",
                "Full setup (all settings)",
                "Telegram only",
            ],
            0,
        )
    } else {
        (
            "How would you like to set up Orca?",
            vec![
                "Quick setup (recommended)",
                "Full setup (all settings)",
                "Telegram only",
            ],
            0,
        )
    };

    let idx = Select::with_theme(theme)
        .with_prompt(prompt)
        .items(&items)
        .default(default_idx)
        .interact()?;

    Ok(match idx {
        0 => SetupMode::Quick,
        1 => SetupMode::Full,
        _ => SetupMode::TelegramOnly,
    })
}

const ZAI_CODING_PLAN_BASE: &str = "https://api.z.ai/api/coding/paas/v4";
const ZAI_STANDARD_BASE: &str = "https://api.z.ai/api/paas/v4";

fn choose_provider(theme: &ColorfulTheme) -> anyhow::Result<ProviderPreset> {
    let providers = [
        ProviderPreset::OpenRouter,
        ProviderPreset::OpenAi,
        ProviderPreset::Anthropic,
        ProviderPreset::Xai,
        ProviderPreset::Zai,
        ProviderPreset::Mistral,
        ProviderPreset::GithubCopilot,
        ProviderPreset::GoogleVertex,
        ProviderPreset::AzureOpenAi,
        ProviderPreset::Ollama,
        ProviderPreset::HermesGateway,
        ProviderPreset::Custom,
    ];
    let labels: Vec<&str> = providers.iter().map(|p| p.label()).collect();
    let idx = Select::with_theme(theme)
        .with_prompt("Choose provider")
        .items(&labels)
        .default(0)
        .interact()?;
    Ok(providers[idx])
}

fn choose_zai_endpoint(theme: &ColorfulTheme, current: Option<&str>) -> anyhow::Result<String> {
    let choices = [
        "Coding Plan endpoint (recommended for GLM-4.5/4.6/4.7/5/5.1)",
        "Standard endpoint (regular Z.AI API)",
    ];

    let current_norm = current
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_lowercase();
    let default_idx = if current_norm == ZAI_STANDARD_BASE.to_lowercase() {
        1
    } else {
        0
    };

    let idx = Select::with_theme(theme)
        .with_prompt("Choose your Z.AI endpoint")
        .items(&choices)
        .default(default_idx)
        .interact()?;

    Ok(if idx == 1 {
        ZAI_STANDARD_BASE.to_string()
    } else {
        ZAI_CODING_PLAN_BASE.to_string()
    })
}

fn models_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/models") {
        return trimmed.to_string();
    }
    format!("{trimmed}/models")
}

fn extract_model_names(payload: &Value) -> Vec<String> {
    let mut out = BTreeSet::new();

    let push_obj = |obj: &serde_json::Map<String, Value>, out: &mut BTreeSet<String>| {
        if let Some(id) = obj.get("id").and_then(Value::as_str) {
            let t = id.trim();
            if !t.is_empty() {
                out.insert(t.to_string());
            }
        }
        if let Some(name) = obj.get("name").and_then(Value::as_str) {
            let t = name.trim();
            if !t.is_empty() {
                out.insert(t.to_string());
            }
        }
        if let Some(model) = obj.get("model").and_then(Value::as_str) {
            let t = model.trim();
            if !t.is_empty() {
                out.insert(t.to_string());
            }
        }
    };

    if let Some(arr) = payload.get("data").and_then(Value::as_array) {
        for item in arr {
            if let Some(obj) = item.as_object() {
                push_obj(obj, &mut out);
            }
        }
    }

    if let Some(arr) = payload.get("models").and_then(Value::as_array) {
        for item in arr {
            if let Some(obj) = item.as_object() {
                push_obj(obj, &mut out);
            }
        }
    }

    out.into_iter().collect()
}

fn detect_models(
    provider: ProviderPreset,
    base_url: &str,
    api_key: &str,
) -> anyhow::Result<Vec<String>> {
    let endpoint = models_endpoint(base_url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()?;

    let mut req = client.get(&endpoint);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key.trim());
    }

    if matches!(provider, ProviderPreset::Anthropic) {
        req = req.header("x-api-key", api_key.trim());
        req = req.header("anthropic-version", "2023-06-01");
    }

    let resp = req.send()?;
    if !resp.status().is_success() {
        anyhow::bail!("{} {}", resp.status(), endpoint);
    }

    let payload: Value = resp.json()?;
    Ok(extract_model_names(&payload))
}

fn prompt_for_model(
    theme: &ColorfulTheme,
    default_model: String,
    detected_models: Vec<String>,
) -> anyhow::Result<String> {
    if detected_models.is_empty() {
        let value: String = Input::with_theme(theme)
            .with_prompt("Model id")
            .default(default_model)
            .interact_text()?;
        return Ok(value.trim().to_string());
    }

    let mut items = detected_models;
    if !items.iter().any(|m| m == &default_model) {
        items.insert(0, default_model.clone());
    }
    items.push("<Enter model id manually>".to_string());

    let default_idx = items.iter().position(|m| m == &default_model).unwrap_or(0);

    let idx = Select::with_theme(theme)
        .with_prompt("Detected models (choose one)")
        .items(&items)
        .default(default_idx)
        .interact()?;

    if idx == items.len() - 1 {
        let value: String = Input::with_theme(theme)
            .with_prompt("Model id")
            .default(default_model)
            .interact_text()?;
        return Ok(value.trim().to_string());
    }

    Ok(items[idx].clone())
}

fn setup_provider_and_model(theme: &ColorfulTheme, cfg: &mut OrcaConfig) -> anyhow::Result<()> {
    let provider = choose_provider(theme)?;

    let has_existing_key = cfg
        .llm
        .api_key
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let key_prompt = if has_existing_key {
        "API key (empty = keep existing key in config)"
    } else {
        provider.key_prompt()
    };

    let api_key = Password::with_theme(theme)
        .with_prompt(key_prompt)
        .allow_empty_password(true)
        .interact()?;

    if !api_key.trim().is_empty() {
        cfg.llm.api_key = Some(api_key.trim().to_string());
    }

    let base_url_default = if matches!(provider, ProviderPreset::Zai) {
        choose_zai_endpoint(theme, cfg.llm.base_url.as_deref())?
    } else {
        cfg.llm
            .base_url
            .clone()
            .unwrap_or_else(|| provider.default_base_url().to_string())
    };

    let base_url: String = Input::with_theme(theme)
        .with_prompt("OpenAI-compatible base URL")
        .default(base_url_default)
        .interact_text()?;

    if !base_url.trim().is_empty() {
        cfg.llm.base_url = Some(base_url.trim().to_string());
    }

    let default_model = cfg
        .llm
        .model
        .clone()
        .unwrap_or_else(|| provider.default_model().to_string());

    let effective_key = cfg.llm.api_key.clone().unwrap_or_default();
    let detected_models = if effective_key.trim().is_empty() {
        Vec::new()
    } else {
        println!("Detecting models for {}...", provider.label());
        match detect_models(
            provider,
            cfg.llm.base_url.as_deref().unwrap_or(""),
            &effective_key,
        ) {
            Ok(models) => {
                if models.is_empty() {
                    println!(
                        "No models were returned by this endpoint/key. You can enter one manually."
                    );
                } else {
                    println!("Detected {} model(s).", models.len());
                }
                models
            }
            Err(err) => {
                println!(
                    "Could not auto-detect models ({err}). You can still enter model id manually."
                );
                Vec::new()
            }
        }
    };

    let model = prompt_for_model(theme, default_model, detected_models)?;

    if !model.trim().is_empty() {
        cfg.llm.model = Some(model.trim().to_string());
    }

    Ok(())
}

fn setup_telegram(theme: &ColorfulTheme) -> anyhow::Result<()> {
    let has_existing = telegram_token_from_keyring().is_some();
    let mut choices = vec!["Skip for now", "Enter Telegram bot token now"];
    if has_existing {
        choices.push("Keep existing token in keyring");
    }

    let idx = Select::with_theme(theme)
        .with_prompt("Telegram setup")
        .items(&choices)
        .default(if has_existing { 2 } else { 0 })
        .interact()?;

    if idx == 0 {
        println!("Skipped Telegram setup. You can run `orca setup` again later.");
        return Ok(());
    }

    if has_existing && idx == 2 {
        println!("Keeping existing Telegram token in OS keyring.");
        return Ok(());
    }

    let tg = Password::with_theme(theme)
        .with_prompt("Telegram bot token")
        .allow_empty_password(false)
        .interact()?;

    telegram_token_store_keyring(tg.trim()).context("store telegram token in keyring")?;
    println!("Telegram bot token stored in OS keyring (orca / telegram_bot_token).");
    println!("Tip: run `orca telegram qr` to open your bot quickly.");
    println!(
        "Tip: orcad reads ORCA_TELEGRAM_BOT_TOKEN from environment when launching the gateway."
    );

    Ok(())
}

pub fn cmd_setup_interactive(
    cmd_install: impl FnOnce() -> anyhow::Result<()>,
    cmd_start: impl FnOnce() -> anyhow::Result<()>,
    daemon_available: bool,
) -> anyhow::Result<()> {
    let theme = ColorfulTheme::default();
    println!("Orca setup wizard\n");

    let mut cfg = OrcaConfig::load().unwrap_or_default();
    let is_existing = OrcaConfig::path().map(|p| p.exists()).unwrap_or(false);
    let mode = choose_setup_mode(&theme, is_existing)?;

    if !matches!(mode, SetupMode::TelegramOnly) {
        let default_port = cfg.server.port.to_string();
        let port_input: String = Input::with_theme(&theme)
            .with_prompt("Companion server port")
            .default(default_port)
            .interact_text()?;
        cfg.server.port = port_input.parse::<u16>().unwrap_or(3001);

        let default_ws = cfg
            .server
            .workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .map(|h| h.join("Desktop"))
                    .unwrap_or_else(|| PathBuf::from("."))
            });

        let ws: String = Input::with_theme(&theme)
            .with_prompt("Workspace root (WORKSPACE_ROOT)")
            .default(default_ws.display().to_string())
            .interact_text()?;

        let ws = ws.trim();
        if !ws.is_empty() {
            cfg.server.workspace = Some(PathBuf::from(ws));
        } else {
            cfg.server.workspace = None;
        }

        if cfg.bridge.token.is_some() {
            let keep = Confirm::with_theme(&theme)
                .with_prompt("Keep existing bridge token?")
                .default(true)
                .interact()?;
            if !keep {
                let tok = uuid::Uuid::new_v4().to_string();
                cfg.bridge.token = Some(tok.clone());
                if let Err(e) = bridge_token_store_keyring(&tok) {
                    println!("Warning: could not store token in keyring ({e})");
                }
            }
        } else {
            let tok = uuid::Uuid::new_v4().to_string();
            cfg.bridge.token = Some(tok.clone());
            match bridge_token_store_keyring(&tok) {
                Ok(()) => println!("Generated bridge token (OS keyring + config)"),
                Err(e) => println!("Warning: could not store token in keyring ({e})"),
            }
        }

        setup_provider_and_model(&theme, &mut cfg)?;

        if matches!(mode, SetupMode::Full) {
            let hn = cfg
                .harness
                .node_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let node_path: String = Input::with_theme(&theme)
                .with_prompt("Node binary path (optional; default: node on PATH)")
                .default(hn)
                .allow_empty(true)
                .interact_text()?;
            cfg.harness.node_path = if node_path.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(node_path.trim()))
            };

            let hs = cfg
                .harness
                .script
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let script: String = Input::with_theme(&theme)
                .with_prompt("Harness script path (optional; default: next to orcad)")
                .default(hs)
                .allow_empty(true)
                .interact_text()?;
            cfg.harness.script = if script.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(script.trim()))
            };
        }
    }

    setup_telegram(&theme)?;

    cfg.save()?;
    println!("\nWrote {}", OrcaConfig::path()?.display());

    if !matches!(mode, SetupMode::TelegramOnly) {
        if daemon_available {
            if Confirm::with_theme(&theme)
                .with_prompt("Run `orca install` (daemon registration) now?")
                .default(true)
                .interact()?
            {
                cmd_install()?;
            }

            if Confirm::with_theme(&theme)
                .with_prompt("Start daemon now?")
                .default(true)
                .interact()?
            {
                cmd_start()?;
            }
        } else {
            println!("\nDaemon binary not found (`orcad`). Skipping daemon install/start prompts.");
            println!(
                "Install orcad later, then run `orca install` (or set ORCAD_PATH to your orcad binary)."
            );
        }
    }

    println!("\nNext: `orca doctor` and `orca status`. Then run `orca` to chat.");
    Ok(())
}
