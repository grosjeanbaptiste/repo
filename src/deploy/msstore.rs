//! Microsoft Store (Windows): publish a built `.msix`; prod -> `production`, test -> `flight`.
use super::ctx::Ctx;
use super::find;
use super::target::Env;
use super::tool::plain;
use anyhow::{Context, Result, bail};

pub fn upload(c: &mut Ctx) -> Result<()> {
    if !c.tool.on_path("msstore") {
        bail!("msstore not found — MS Store publish needs Windows + Partner Center creds.");
    }
    let build = c.root.join("build");
    let Some(msix) = find::deep(&build, "msix") else {
        bail!(
            "no .msix under {} (package with `dart run msix:create`).",
            build.display()
        );
    };
    let channel = if c.env == Env::Prod {
        "production"
    } else {
        "flight"
    };
    let args = [
        "publish".to_string(),
        msix.display().to_string(),
        "--channel".to_string(),
        channel.to_string(),
    ];
    c.tool
        .run(&plain("msstore", &args))
        .context("publishing to the Microsoft Store")
}

#[cfg(test)]
mod tests {
    include!("msstore_tests.rs");
}
