//! `repo release` (port of dotfiles `project release`): run the project's release script with the profile and `.envrc` loaded.
pub mod args;
pub mod command;
pub mod launch;
pub mod script;

use anyhow::{Context, Result, anyhow, bail};
use args::Opts;
use launch::{Launch, argv};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Runs the audit of a directory, writing its report and its warnings.
pub type Audit<'a> = &'a dyn Fn(&Path, &mut dyn Write, &mut dyn Write) -> Result<()>;

pub struct Ports<'a> {
    /// The machine profile, sourced before the project's `.envrc`.
    pub profile: PathBuf,
    pub audit: Audit<'a>,
    pub launch: &'a dyn Launch,
}

/// Finds the script, audits first when asked, then hands over to the script. Returns only on failure.
pub fn run(opts: &Opts, ports: &Ports, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    let dir = Path::new(opts.path.as_deref().unwrap_or("."));
    if !dir.is_dir() {
        bail!("release: path is not a directory: {}", dir.display());
    }
    let target = dir
        .canonicalize()
        .with_context(|| format!("resolving {}", dir.display()))?;
    let found =
        script::resolve(&target, opts.script.as_deref()).map_err(|e| anyhow!("release: {e}"))?;
    let command = script::command(&found).map_err(|e| anyhow!("release: {e}"))?;
    if opts.require_audit {
        (ports.audit)(&target, out, err).map_err(|e| {
            anyhow!(
                "release: audit failed for {} — aborting: {e:#}",
                target.display()
            )
        })?;
        writeln!(out).context("writing the report")?;
    }
    ports
        .launch
        .launch(&argv(&ports.profile, &target, &command, &opts.forward))
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
