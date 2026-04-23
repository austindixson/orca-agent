#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::PathBuf;

pub fn resolve_orcad() -> anyhow::Result<PathBuf> {
    if let Ok(p) = std::env::var("ORCAD_PATH") {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Ok(pb);
        }
    }
    if let Ok(p) = which::which("orcad") {
        return Ok(p);
    }
    let exe = std::env::current_exe()?;
    if let Some(dir) = exe.parent() {
        let sibling = dir.join("orcad");
        if cfg!(windows) {
            let sibling_exe = dir.join("orcad.exe");
            if sibling_exe.is_file() {
                return Ok(sibling_exe);
            }
        }
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    anyhow::bail!("orcad not found — set ORCAD_PATH or add orcad to PATH")
}
