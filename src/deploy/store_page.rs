//! `--store-page`: push only the store listing from `<project>/stores` (no binary, no review submission).
use super::ctx::Ctx;
use super::stores;
use super::target::{Choice, Target};
use super::tool::{Call, plain};
use anyhow::{Context, Result, bail};
use std::path::Path;

fn ios(c: &mut Ctx, stores: &Path, dry_run: bool) -> Result<()> {
    let Some(bundle) = c.vars.var("ASC_BUNDLE_ID") else {
        bail!("--store-page ios needs ASC_BUNDLE_ID (per-project, e.g. .envrc).");
    };
    if dry_run {
        return writeln!(
            c.out,
            "STORES_DIR={} asc_upload.py (App Store page of {bundle}, no submission)",
            stores.display()
        )
        .context("writing the plan");
    }
    run_script(
        c,
        "asc_upload.py",
        vec![("STORES_DIR".into(), stores.display().to_string())],
    )
}

fn android(c: &mut Ctx, stores: &Path, dry_run: bool) -> Result<()> {
    let Some(package) = stores::android_package(c.vars, c.root)? else {
        bail!("cannot resolve the Android package (set PLAY_PACKAGE_NAME).");
    };
    let account = c.vars.path_or(
        "PLAY_SERVICE_ACCOUNT_PATH",
        ".config/dotfiles/play-service-account.json",
    );
    if !account.is_file() {
        bail!(
            "no Play service account at {} (dotfiles secret apply).",
            account.display()
        );
    }
    if dry_run {
        return writeln!(
            c.out,
            "STORES_DIR={} play_upload.py (Play listing of {package}, no bundle)",
            stores.display()
        )
        .context("writing the plan");
    }
    let env = vec![
        ("STORES_DIR".to_string(), stores.display().to_string()),
        ("PLAY_PACKAGE_NAME".to_string(), package),
        (
            "PLAY_SERVICE_ACCOUNT_PATH".to_string(),
            account.display().to_string(),
        ),
    ];
    run_script(c, "play_upload.py", env)
}

fn run_script(c: &mut Ctx, script: &str, env: Vec<(String, String)>) -> Result<()> {
    let py = stores::python(c.vars, c.tool, c.err)?.display().to_string();
    let args = [stores::scripts_dir(c.vars)
        .join(script)
        .display()
        .to_string()];
    c.tool
        .run(&Call {
            env: &env,
            ..plain(&py, &args)
        })
        .with_context(|| format!("pushing the store page with {script}"))
}

pub fn push(c: &mut Ctx, choice: Choice, dry_run: bool) -> Result<()> {
    let target = match choice {
        Choice::One(t @ (Target::Ios | Target::Android)) => t,
        Choice::One(t) => bail!(
            "--store-page applies to ios and android, not '{}'.",
            t.name()
        ),
        Choice::All => bail!("--store-page applies to ios and android, not 'all'."),
    };
    let stores = c.root.join("stores");
    if !stores.is_dir() {
        bail!(
            "--store-page reads {}, which does not exist.",
            stores.display()
        );
    }
    if target == Target::Ios {
        ios(c, &stores, dry_run)
    } else {
        android(c, &stores, dry_run)
    }
}

#[cfg(test)]
mod tests {
    include!("store_page_tests.rs");
}
