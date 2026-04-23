//! Interactive `orca setup` wizard (Hermes-style).

use std::path::PathBuf;

use anyhow::Context;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};

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
    Custom,
}

impl ProviderPreset {
    fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter (recommended)",
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Custom => "Custom OpenAI-compatible endpoint",
        }
    }

    fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Custom => "http://localhost:11434/v1",
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::OpenRouter => "openai/gpt-4o-mini",
            Self::OpenAi => "gpt-4o-mini",
            Self::Anthropic => "claude-3-5-sonnet-latest",
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
            Self::Custom => "API key (optional; leave empty if your endpoint does not require one)",
        }
    }
}

/// Non-interactive: merge `PORT`, `WORKSPACE_ROOT`, `OPENROUTER_API_KEY`, `ORCA_MODEL`, `ORCA_LLM_BASE_URL` into config and save.
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
    if let Ok(k) = std::env::var("OPENROUTER_API_KEY") {
        if !k.trim().is_empty() {
            cfg.llm.api_key = Some(k);
        }
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

fn choose_provider(theme: &ColorfulTheme) -> anyhow::Result<ProviderPreset> {
    let providers = [
        ProviderPreset::OpenRouter,
        ProviderPreset::OpenAi,
        ProviderPreset::Anthropic,
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

    let base_url: String = Input::with_theme(theme)
        .with_prompt("OpenAI-compatible base URL")
        .default(
            cfg.llm
                .base_url
                .clone()
                .unwrap_or_else(|| provider.default_base_url().to_string()),
        )
        .interact_text()?;

    if !base_url.trim().is_empty() {
        cfg.llm.base_url = Some(base_url.trim().to_string());
    }

    let model: String = Input::with_theme(theme)
        .with_prompt("Model id")
        .default(
            cfg.llm
                .model
                .clone()
                .unwrap_or_else(|| provider.default_model().to_string()),
        )
        .interact_text()?;

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
    }

    println!("\nNext: `orca doctor` and `orca status`. Then run `orca` to chat.");
    Ok(())
}
