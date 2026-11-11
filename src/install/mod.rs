//! `repo install NAME` (port of dotfiles `project install`): install a registered Cargo project on PATH.
pub mod check;
pub mod command;
pub mod manifest;

use crate::registry::file::Entry;
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result};
use std::path::Path;

pub struct Opts {
    pub name: String,
    pub package: Option<String>,
    pub bin: Option<String>,
    pub force: bool,
}

/// Checks the project with `probe`, then compiles it with `live` (through the heavy queue).
/// Prints the confirmation line to `out`.
pub fn run(
    opts: &Opts,
    entries: &[Entry],
    probe: &dyn Exec,
    live: &dyn Exec,
    out: &mut dyn std::io::Write,
) -> Result<()> {
    let root = check::project(entries, &opts.name, probe)?;
    let from = match &opts.package {
        Some(package) => member(probe, &root, &opts.name, package)?,
        None => root.clone(),
    };
    let from = from.to_string_lossy();
    let mut args = vec![
        "heavy", "run", "--", "cargo", "install", "--path", &from, "--locked",
    ];
    args.extend(opts.bin.iter().flat_map(|b| ["--bin", b.as_str()]));
    args.extend(opts.force.then_some("--force"));
    live.run("dotfiles", &args, None)
        .with_context(|| format!("Cargo installation failed for {}", opts.name))?;
    let what = opts
        .bin
        .as_ref()
        .or(opts.package.as_ref())
        .unwrap_or(&opts.name);
    writeln!(out, "Installed {what} from {}", root.display()).context("writing the confirmation")
}

fn member(probe: &dyn Exec, root: &Path, name: &str, package: &str) -> Result<std::path::PathBuf> {
    let args = ["metadata", "--no-deps", "--format-version", "1", "--locked"];
    let json = probe
        .run("cargo", &args, Some(root))
        .with_context(|| format!("cargo metadata failed for {name}"))?;
    manifest::member_dir(&json, package, root)
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
