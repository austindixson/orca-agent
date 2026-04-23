//! Scheduled task at logon (no admin): `schtasks /Create /SC ONLOGON ...`

use std::path::PathBuf;
use std::process::Command;

const TASK_NAME: &str = "OrcaDaemon";

pub fn log_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Orca")
        .join("Logs")
}

pub fn create_task(orcad: &std::path::Path) -> anyhow::Result<()> {
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir)?;
    let tr = format!(r#""{}" --supervise"#, orcad.display());

    let status = Command::new("schtasks")
        .args([
            "/Create",
            "/F",
            "/SC",
            "ONLOGON",
            "/RL",
            "LIMITED",
            "/TN",
            TASK_NAME,
            "/TR",
            &tr,
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("schtasks /Create failed (exit {:?})", status.code());
    }
    Ok(())
}

pub fn delete_task() -> anyhow::Result<()> {
    let _ = Command::new("schtasks")
        .args(["/Delete", "/F", "/TN", TASK_NAME])
        .status();
    Ok(())
}

pub fn run_task() -> anyhow::Result<()> {
    let status = Command::new("schtasks")
        .args(["/Run", "/TN", TASK_NAME])
        .status()?;
    if !status.success() {
        anyhow::bail!("schtasks /Run failed");
    }
    Ok(())
}

pub fn is_installed() -> bool {
    let out = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}
