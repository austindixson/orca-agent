//! `orca` — CLI for daemon install, status, logs, chat, exec, doctor.

mod config;
mod http_client;
mod installer;
mod setup;
mod tui;

use std::io::{self, BufRead};
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::config::{
    bridge_token_from_keyring, bridge_token_store_keyring, telegram_token_from_keyring, OrcaConfig,
};
use crate::http_client::{get_json, post_json};
use crate::setup::{cmd_setup_defaults, cmd_setup_interactive};

#[derive(Parser)]
#[command(name = "orca", about = "Orca Coder — daemon & bridge CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Telegram onboarding (QR deep link to your bot)
    Telegram {
        #[command(subcommand)]
        action: TelegramCmd,
    },
    /// Install user daemon (LaunchAgent / scheduled task)
    Install,
    /// Remove daemon registration
    Uninstall,
    /// Start daemon (launchctl / schtasks)
    Start,
    /// Stop daemon (best-effort)
    Stop,
    /// Restart daemon
    Restart,
    /// Health + bridge + gateway status
    Status,
    /// Print or follow orcad.log
    Logs {
        #[arg(short, long)]
        follow: bool,
    },
    /// Send a message through the harness (POST /api/harness/chat)
    Chat { text: String },
    /// POST /api/canvas/execute (Hermes-style)
    Exec {
        tool: String,
        /// JSON arguments (default {})
        args: Option<String>,
    },
    /// POST /api/orchestrator/reply — hand off a completion to the lead orchestrator (Orca UI must be open)
    Reply {
        /// Parent orchestrator widget tile id (defaults to ORCA_PARENT_TILE_ID)
        #[arg(long)]
        tile: Option<String>,
        /// Session id for logging (defaults to ORCA_PARENT_SESSION_ID)
        #[arg(long)]
        session: Option<String>,
        /// Handoff role label
        #[arg(long, default_value = "external")]
        role: String,
        /// Child tile id if this completed a specific Hermes/worker tile
        #[arg(long)]
        child_tile: Option<String>,
        /// Final summary text
        text: String,
    },
    /// Port, token, node, config checks
    Doctor,
    /// Interactive wizard — model, keys, workspace, daemon (Hermes-style)
    Setup {
        /// Non-interactive: merge PORT, WORKSPACE_ROOT, OPENROUTER_API_KEY, ORCA_MODEL, ORCA_LLM_BASE_URL
        #[arg(long)]
        defaults: bool,
    },
    /// Full-screen terminal chat UI
    Tui,
}

#[derive(Subcommand)]
enum TelegramCmd {
    /// Print a terminal QR that opens your bot in Telegram (uses ORCA_TELEGRAM_BOT_TOKEN or keyring)
    Qr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Telegram { action } => match action {
            TelegramCmd::Qr => cmd_telegram_qr().await?,
        },
        Commands::Install => cmd_install()?,
        Commands::Uninstall => cmd_uninstall()?,
        Commands::Start => cmd_start()?,
        Commands::Stop => cmd_stop()?,
        Commands::Restart => {
            let _ = cmd_stop();
            cmd_start()?;
        }
        Commands::Status => cmd_status().await?,
        Commands::Logs { follow } => cmd_logs(follow)?,
        Commands::Chat { text } => cmd_chat(text).await?,
        Commands::Exec { tool, args } => cmd_exec(tool, args).await?,
        Commands::Reply {
            tile,
            session,
            role,
            child_tile,
            text,
        } => cmd_reply(tile, session, role, child_tile, text).await?,
        Commands::Doctor => cmd_doctor().await?,
        Commands::Setup { defaults } => {
            if defaults {
                cmd_setup_defaults()?;
            } else {
                let daemon_available = installer::resolve_orcad().is_ok();
                cmd_setup_interactive(cmd_install, cmd_start, daemon_available)?;
            }
        }
        Commands::Tui => tui::run_tui().await?,
    }

    Ok(())
}

fn merge_token(mut cfg: OrcaConfig) -> OrcaConfig {
    if cfg.bridge.token.is_none() {
        if let Some(t) = bridge_token_from_keyring() {
            cfg.bridge.token = Some(t);
        }
    }
    if let Ok(t) = std::env::var("CANVAS_BRIDGE_TOKEN") {
        if !t.trim().is_empty() {
            cfg.bridge.token = Some(t);
        }
    }
    if let Ok(t) = std::env::var("ORCA_BRIDGE_TOKEN") {
        if !t.trim().is_empty() {
            cfg.bridge.token = Some(t);
        }
    }
    cfg
}

pub(crate) fn cmd_install() -> anyhow::Result<()> {
    let orcad = installer::resolve_orcad()?;
    let mut cfg = OrcaConfig::load().unwrap_or_default();

    if cfg.bridge.token.is_none() {
        let tok = uuid::Uuid::new_v4().to_string();
        cfg.bridge.token = Some(tok.clone());
        match bridge_token_store_keyring(&tok) {
            Ok(()) => println!("Generated bridge token (OS keyring + ~/.orca/config.toml)"),
            Err(e) => println!(
                "Warning: could not store token in keyring ({}); token only in ~/.orca/config.toml",
                e
            ),
        }
    }

    cfg.save()?;

    #[cfg(target_os = "macos")]
    {
        use installer::macos;
        macos::write_plist(&orcad)?;
        macos::bootstrap()?;
        println!("Installed LaunchAgent: {}", macos::plist_path().display());
    }
    #[cfg(target_os = "windows")]
    {
        use installer::windows;
        windows::create_task(&orcad)?;
        println!("Installed scheduled task: OrcaDaemon");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("install is only supported on macOS and Windows");
    }

    println!("orcad: {}", orcad.display());
    Ok(())
}

fn cmd_uninstall() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use installer::macos;
        macos::unload()?;
        println!("Removed LaunchAgent");
    }
    #[cfg(target_os = "windows")]
    {
        installer::windows::delete_task()?;
        println!("Removed scheduled task");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("uninstall is only supported on macOS and Windows");
    }
    Ok(())
}

pub(crate) fn cmd_start() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        installer::macos::kickstart()?;
        println!("kickstarted com.orca.daemon");
    }
    #[cfg(target_os = "windows")]
    {
        installer::windows::run_task()?;
        println!("ran scheduled task OrcaDaemon");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("start is only supported on macOS and Windows");
    }
    Ok(())
}

fn cmd_stop() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("launchctl")
            .args(["kill", "SIGTERM", "com.orca.daemon"])
            .status();
        println!("sent SIGTERM to com.orca.daemon (if running)");
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", "orcad.exe", "/F"])
            .status();
        println!("taskkill orcad.exe (best-effort)");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("stop is only supported on macOS and Windows");
    }
    Ok(())
}

async fn cmd_telegram_qr() -> anyhow::Result<()> {
    let token = std::env::var("ORCA_TELEGRAM_BOT_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(telegram_token_from_keyring)
        .context("No Telegram bot token — set ORCA_TELEGRAM_BOT_TOKEN or run `orca setup` to save one in the keyring")?;

    let url = format!("https://api.telegram.org/bot{}/getMe", token.trim());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let res = client.get(&url).send().await?;
    let v: Value = res.json().await?;
    if v.get("ok").and_then(|x| x.as_bool()) != Some(true) {
        let err = v
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("getMe failed");
        anyhow::bail!("{}", err);
    }
    let username = v["result"]["username"]
        .as_str()
        .context("getMe response missing username")?;
    let open = format!("https://t.me/{username}");
    println!("Orca Telegram — scan with your phone to open the bot in Telegram:\n");
    qr2term::print_qr(&open)?;
    println!("\n{open}\n");
    Ok(())
}

async fn cmd_status() -> anyhow::Result<()> {
    let cfg = merge_token(OrcaConfig::load()?);
    let base = cfg.base_url();
    println!("Orca bridge — {}\n", base);

    #[cfg(target_os = "macos")]
    println!(
        "launchd: {}",
        if installer::macos::is_installed() {
            "installed"
        } else {
            "not installed"
        }
    );
    #[cfg(target_os = "windows")]
    println!(
        "task: {}",
        if installer::windows::is_installed() {
            "installed"
        } else {
            "not installed"
        }
    );

    match get_json(&cfg, "/api/health").await {
        Ok((s, _b)) => println!("health:   {} ({})", if s == 200 { "ok" } else { "fail" }, s),
        Err(e) => println!("health:   error — {}", e),
    }
    match get_json(&cfg, "/api/canvas/bridge-status").await {
        Ok((s, b)) => println!("bridge:   {} — {}", s, b),
        Err(e) => println!("bridge:   error — {}", e),
    }
    match get_json(&cfg, "/api/gateway/status").await {
        Ok((s, b)) => println!("gateway:  {} — {}", s, b),
        Err(e) => println!("gateway:  error — {}", e),
    }

    Ok(())
}

fn log_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Logs/Orca/orcad.log")
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Orca")
            .join("Logs")
            .join("orcad.log")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".orca/logs/orcad.log")
    }
}

fn cmd_logs(follow: bool) -> anyhow::Result<()> {
    let p = log_path();
    if !p.exists() {
        println!("(no log file yet) {}", p.display());
        return Ok(());
    }
    if follow {
        let f = std::fs::File::open(&p).with_context(|| p.display().to_string())?;
        let reader = io::BufReader::new(f);
        for line in reader.lines() {
            println!("{}", line?);
        }
        println!("(follow mode: full file printed; use `tail -f` for live follow)");
    } else {
        let s = std::fs::read_to_string(&p).with_context(|| p.display().to_string())?;
        print!("{}", s);
    }
    Ok(())
}

async fn cmd_chat(text: String) -> anyhow::Result<()> {
    let cfg = merge_token(OrcaConfig::load()?);
    let (st, body) = post_json(&cfg, "/api/harness/chat", json!({ "text": text })).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "status": st, "body": body }))?
    );
    Ok(())
}

async fn cmd_exec(tool: String, args: Option<String>) -> anyhow::Result<()> {
    let cfg = merge_token(OrcaConfig::load()?);
    let arguments: Value = if let Some(s) = args {
        serde_json::from_str(&s).unwrap_or(json!({}))
    } else {
        json!({})
    };
    let (st, body) = post_json(
        &cfg,
        "/api/canvas/execute",
        json!({ "tool": tool, "arguments": arguments }),
    )
    .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "status": st, "body": body }))?
    );
    Ok(())
}

async fn cmd_reply(
    tile: Option<String>,
    session: Option<String>,
    role: String,
    child_tile: Option<String>,
    text: String,
) -> anyhow::Result<()> {
    let cfg = merge_token(OrcaConfig::load()?);
    let parent_tile_id = tile
        .or_else(|| std::env::var("ORCA_PARENT_TILE_ID").ok())
        .filter(|s| !s.trim().is_empty())
        .context("Set --tile or ORCA_PARENT_TILE_ID to the orchestrator widget tile id")?;
    let session_id = session
        .or_else(|| std::env::var("ORCA_PARENT_SESSION_ID").ok())
        .filter(|s| !s.trim().is_empty());
    let mut body = json!({
        "parent_tile_id": parent_tile_id.trim(),
        "text": text,
        "role": role,
    });
    if let Some(ct) = child_tile.filter(|s| !s.trim().is_empty()) {
        body["child_tile_id"] = json!(ct);
    }
    if let Some(sid) = session_id {
        body["session_id"] = json!(sid);
    }
    let (st, b) = post_json(&cfg, "/api/orchestrator/reply", body).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "status": st, "body": b }))?
    );
    Ok(())
}

async fn cmd_doctor() -> anyhow::Result<()> {
    let cfg = merge_token(OrcaConfig::load()?);
    println!("config:   {:?}", OrcaConfig::path()?);
    println!("base_url: {}", cfg.base_url());
    println!(
        "token:    {}",
        if cfg.bridge.token.is_some() {
            "(set)"
        } else {
            "(missing — run orca install)"
        }
    );
    match which::which("node") {
        Ok(p) => println!("node:     {}", p.display()),
        Err(_) => println!("node:     NOT FOUND"),
    }
    match which::which("orcad") {
        Ok(p) => println!("orcad:    {}", p.display()),
        Err(_) => println!("orcad:    NOT FOUND (set ORCAD_PATH)"),
    }

    let client = reqwest::Client::new();
    let port = cfg.server.port;
    let probe = client
        .get(format!("http://127.0.0.1:{}/api/health", port))
        .send()
        .await;
    match probe {
        Ok(r) => println!("probe:    /api/health → {}", r.status()),
        Err(e) => println!("probe:    failed — {}", e),
    }

    Ok(())
}
