//! Shared config for `orca` CLI — `~/.orca/config.toml`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrcaConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub bridge: BridgeSection,
    #[serde(default)]
    pub harness: HarnessSection,
    /// Preserved for headless harness; optional.
    #[serde(default)]
    pub llm: LlmSection,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerSection {
    #[serde(default = "default_port")]
    pub port: u16,
    /// Companion server workspace root (WORKSPACE_ROOT); default is process cwd when orcad starts.
    #[serde(default)]
    pub workspace: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BridgeSection {
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HarnessSection {
    #[serde(default)]
    pub script: Option<PathBuf>,
    /// Override `node` binary (same as orcad `[harness] node_path`).
    #[serde(default)]
    pub node_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LlmSection {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

fn default_port() -> u16 {
    3001
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            port: default_port(),
            workspace: None,
        }
    }
}

impl Default for BridgeSection {
    fn default() -> Self {
        Self { token: None }
    }
}

impl Default for HarnessSection {
    fn default() -> Self {
        Self {
            script: None,
            node_path: None,
        }
    }
}

impl Default for OrcaConfig {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            bridge: BridgeSection::default(),
            harness: HarnessSection::default(),
            llm: LlmSection::default(),
        }
    }
}

impl OrcaConfig {
    pub fn dir() -> anyhow::Result<PathBuf> {
        Ok(dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("no home directory"))?
            .join(".orca"))
    }

    pub fn path() -> anyhow::Result<PathBuf> {
        Ok(Self::dir()?.join("config.toml"))
    }

    pub fn load() -> anyhow::Result<Self> {
        let p = Self::path()?;
        if !p.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&p)?;
        Ok(toml::from_str(&s)?)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::dir()?;
        std::fs::create_dir_all(&dir)?;
        let p = Self::path()?;
        let s = toml::to_string_pretty(self)?;
        std::fs::write(&p, s)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.server.port)
    }
}

pub fn bridge_token_from_keyring() -> Option<String> {
    keyring::Entry::new("orca", "canvas_bridge_token")
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.trim().is_empty())
}

pub fn bridge_token_store_keyring(token: &str) -> anyhow::Result<()> {
    let e = keyring::Entry::new("orca", "canvas_bridge_token")?;
    e.set_password(token)?;
    Ok(())
}

pub fn telegram_token_from_keyring() -> Option<String> {
    keyring::Entry::new("orca", "telegram_bot_token")
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.trim().is_empty())
}

pub fn telegram_token_store_keyring(token: &str) -> anyhow::Result<()> {
    let e = keyring::Entry::new("orca", "telegram_bot_token")?;
    e.set_password(token)?;
    Ok(())
}
