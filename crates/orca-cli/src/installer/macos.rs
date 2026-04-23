//! User LaunchAgent: `~/Library/LaunchAgents/com.orca.daemon.plist`

use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.orca.daemon";
const PLIST_NAME: &str = "com.orca.daemon.plist";

pub fn launch_agents_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/LaunchAgents")
}

pub fn plist_path() -> PathBuf {
    launch_agents_dir().join(PLIST_NAME)
}

pub fn log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Logs/Orca")
}

pub fn write_plist(orcad: &std::path::Path) -> anyhow::Result<()> {
    let dir = launch_agents_dir();
    std::fs::create_dir_all(&dir)?;
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir)?;

    let stdout = log_dir.join("orcad.launchd.stdout.log");
    let stderr = log_dir.join("orcad.launchd.stderr.log");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>--supervise</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>StandardOutPath</key>
  <string>{out}</string>
  <key>StandardErrorPath</key>
  <string>{err}</string>
</dict>
</plist>
"#,
        label = LABEL,
        exe = orcad.display(),
        out = stdout.display(),
        err = stderr.display()
    );

    std::fs::write(plist_path(), plist)?;
    Ok(())
}

pub fn bootstrap() -> anyhow::Result<()> {
    let uid = std::env::var("UID").unwrap_or_else(|_| {
        let out = Command::new("id")
            .args(["-u"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        out.trim().to_string()
    });
    let domain = format!("gui/{}", uid);
    let p = plist_path();
    let status = Command::new("launchctl")
        .args(["bootstrap", &domain, p.to_str().unwrap()])
        .status()?;
    if !status.success() {
        anyhow::bail!("launchctl bootstrap failed (exit {:?})", status.code());
    }
    Ok(())
}

pub fn bootout() -> anyhow::Result<()> {
    let p = plist_path();
    if p.exists() {
        let status = Command::new("launchctl").args(["bootout", p.to_str().unwrap()]).status();
        let _ = status;
    }
    Ok(())
}

pub fn kickstart() -> anyhow::Result<()> {
    let uid = std::env::var("UID").unwrap_or_else(|_| {
        Command::new("id")
            .args(["-u"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    let target = format!("gui/{}/{}", uid, LABEL);
    let status = Command::new("launchctl")
        .args(["kickstart", "-k", &target])
        .status()?;
    if !status.success() {
        anyhow::bail!("launchctl kickstart failed — try `orca start` after install");
    }
    Ok(())
}

pub fn unload() -> anyhow::Result<()> {
    bootout()?;
    let p = plist_path();
    if p.exists() {
        std::fs::remove_file(&p)?;
    }
    Ok(())
}

pub fn is_installed() -> bool {
    plist_path().exists()
}
