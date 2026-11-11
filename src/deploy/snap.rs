//! Snap Store (Linux): upload a built `.snap`; prod -> `stable`, test -> `beta`.
use super::ctx::Ctx;
use super::find;
use super::target::Env;
use super::tool::{Call, plain};
use anyhow::{Context, Result, bail};

pub fn upload(c: &mut Ctx) -> Result<()> {
    if !c.tool.on_path("snapcraft") {
        bail!("snapcraft not found — Snap publish needs Linux + `snapcraft login`.");
    }
    let credentials = match c.vars.var("SNAPCRAFT_STORE_CREDENTIALS") {
        Some(v) => Some(v),
        None => {
            let args = ["secret", "get", "snapcraft-store-credentials"].map(String::from);
            c.tool
                .capture(&plain("dotfiles", &args))?
                .filter(|v| !v.is_empty())
        }
    };
    let Some(credentials) = credentials else {
        bail!(
            "no Snap Store credentials — enroll with:\n  snapcraft export-login - | dotfiles secret add snapcraft-store-credentials"
        );
    };
    let build = c.root.join("build");
    let Some(snap) = find::deep(&build, "snap") else {
        bail!(
            "no .snap under {} (run `snapcraft pack` first).",
            build.display()
        );
    };
    let channel = if c.env == Env::Prod { "stable" } else { "beta" };
    let args = [
        "upload".to_string(),
        snap.display().to_string(),
        format!("--release={channel}"),
    ];
    let env = [("SNAPCRAFT_STORE_CREDENTIALS".to_string(), credentials)];
    c.tool
        .run(&Call {
            env: &env,
            ..plain("snapcraft", &args)
        })
        .context("uploading the snap")
}

#[cfg(test)]
mod tests {
    include!("snap_tests.rs");
}
