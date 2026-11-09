//! GitHub lookups through `gh` (never reimplemented).
use anyhow::{Context, Result, bail};
use std::process::Command;

pub fn owner() -> Result<String> {
    let out = Command::new("gh")
        .args(["api", "user", "-q", ".login"])
        .output()
        .context("running gh")?;
    if !out.status.success() {
        bail!(
            "gh api user: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn exists(owner: &str, name: &str) -> Result<bool> {
    let out = Command::new("gh")
        .args(["api", &format!("repos/{owner}/{name}")])
        .output()
        .context("running gh")?;
    if out.status.success() {
        return Ok(true);
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("Not Found") {
        return Ok(false);
    }
    bail!("gh api repos/{owner}/{name}: {}", err.trim())
}
