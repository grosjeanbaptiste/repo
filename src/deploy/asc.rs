//! App Store Connect: upload through `xcrun altool`, then (prod) submit the version (port of `project_deploy_asc_*`).
use super::ctx::Ctx;
use super::stores;
use super::target::Env;
use super::tool::plain;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// `ios` or `macos`: the `--type` of altool and the platform of the submission.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Ios,
    Macos,
}

impl Kind {
    fn altool(self) -> &'static str {
        match self {
            Kind::Ios => "ios",
            Kind::Macos => "macos",
        }
    }

    fn platform(self) -> &'static str {
        match self {
            Kind::Ios => "IOS",
            Kind::Macos => "MAC_OS",
        }
    }
}

/// altool looks up `AuthKey_<KEY_ID>.p8` in the private-keys folder: stage the key there.
fn stage_key(c: &Ctx, key_id: &str) -> Result<()> {
    let source = c
        .vars
        .path_or("ASC_PRIVATE_KEY_PATH", ".config/asc/AuthKey.p8");
    if !source.is_file() {
        return Ok(()); // no local key file: altool may find the key by itself
    }
    let dir = c
        .vars
        .path_or("ASC_KEY_STAGE_DIR", ".appstoreconnect/private_keys");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let target = dir.join(format!("AuthKey_{key_id}.p8"));
    fs::copy(&source, &target)
        .with_context(|| format!("copying the key to {}", target.display()))?;
    Ok(())
}

fn submit(c: &mut Ctx, kind: Kind) -> Result<()> {
    let py = stores::python(c.vars, c.tool, c.err)?;
    let script = stores::scripts_dir(c.vars)
        .join("asc_upload.py")
        .display()
        .to_string();
    let args = [script.as_str(), "--submit", "--platform", kind.platform()].map(String::from);
    c.tool
        .run(&plain(&py.display().to_string(), &args))
        .context("submitting the App Store version")
}

/// Uploads the build (it lands in TestFlight after processing); prod then drives the App Store version.
pub fn upload(c: &mut Ctx, kind: Kind, artifact: &Path) -> Result<()> {
    let (Some(key_id), Some(issuer)) = (c.vars.var("ASC_KEY_ID"), c.vars.var("ASC_ISSUER_ID"))
    else {
        bail!("ASC_KEY_ID/ASC_ISSUER_ID not set — source ~/.config/dotfiles/profile.local.sh.");
    };
    let prod = c.env == Env::Prod;
    if prod && c.vars.var("ASC_BUNDLE_ID").is_none() {
        // Refused before the irreversible upload (the bash tool checked after it, and then lost the failure).
        bail!(
            "ASC_BUNDLE_ID must be set for a prod App Store release (per-project, e.g. .envrc). test targets TestFlight only."
        );
    }
    stage_key(c, &key_id)?;
    let file = artifact.display().to_string();
    let args = [
        "altool",
        "--upload-app",
        "--type",
        kind.altool(),
        "--file",
        &file,
        "--apiKey",
        &key_id,
        "--apiIssuer",
        &issuer,
    ]
    .map(String::from);
    c.tool
        .run(&plain("xcrun", &args))
        .context("uploading to App Store Connect")?;
    if prod {
        submit(c, kind)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("asc_tests.rs");
}
