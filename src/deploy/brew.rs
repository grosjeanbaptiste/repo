//! Homebrew: bump the cask to the hosted artifact (the URL must already serve the built `.app`).
use super::ctx::Ctx;
use super::stores;
use super::target::Env;
use super::tool::plain;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// prod bumps the stable cask, test the `@beta` one.
pub fn update(c: &mut Ctx) -> Result<()> {
    let Some(url) = c.vars.var("BREW_APP_URL") else {
        bail!(
            "set BREW_APP_URL to the hosted macOS artifact (Homebrew references a URL, not a local file)."
        );
    };
    let root = c
        .vars
        .var("BREW_ROOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| c.root.join("stores/brew"));
    if !root.join("Casks").is_dir() && !root.join("Formula").is_dir() {
        bail!(
            "no cask/formula under {} (scaffold stores/brew).",
            root.display()
        );
    }
    let version = match c.vars.var("BREW_VERSION") {
        Some(v) => v,
        None => stores::pubspec_version(c.root)?
            .context("cannot resolve version (set BREW_VERSION).")?,
    };
    let channel = if c.env == Env::Prod { "stable" } else { "beta" };
    let py = stores::python(c.vars, c.tool, c.err)?.display().to_string();
    let script = stores::scripts_dir(c.vars)
        .join("update_brew.py")
        .display()
        .to_string();
    let args = [
        script.as_str(),
        "--version",
        &version,
        "--app-url",
        &url,
        "--brew-root",
        &root.display().to_string(),
        "--channel",
        channel,
    ]
    .map(String::from);
    c.tool
        .run(&plain(&py, &args))
        .context("bumping the Homebrew cask")
}

#[cfg(test)]
mod tests {
    include!("brew_tests.rs");
}
