//! Google Play: upload an AAB through the stores script and the managed service account.
use super::ctx::Ctx;
use super::target::{Env, Target};
use super::tool::{Call, plain};
use super::{find, stores};
use anyhow::{Context, Result, bail};

/// prod -> `production` track, test -> `internal`.
pub fn upload(c: &mut Ctx) -> Result<()> {
    let Some(aab) = find::artifact(Target::Android, c.root) else {
        bail!("no .aab under {}/build.", c.root.display());
    };
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
    let track = if c.env == Env::Prod {
        "production"
    } else {
        "internal"
    };
    let py = stores::python(c.vars, c.tool, c.err)?.display().to_string();
    let script = stores::scripts_dir(c.vars)
        .join("play_upload.py")
        .display()
        .to_string();
    let args = [
        script.as_str(),
        "--aab",
        &aab.display().to_string(),
        "--track",
        track,
    ]
    .map(String::from);
    let env = [
        ("PLAY_PACKAGE_NAME".to_string(), package),
        (
            "PLAY_SERVICE_ACCOUNT_PATH".to_string(),
            account.display().to_string(),
        ),
    ];
    let call = Call {
        env: &env,
        ..plain(&py, &args)
    };
    c.tool
        .run(&call)
        .context("uploading the bundle to Google Play")
}

#[cfg(test)]
mod tests {
    include!("play_tests.rs");
}
