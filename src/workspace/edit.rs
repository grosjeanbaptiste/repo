//! `workspace add` and `workspace remove`: edit the `folders` array.
use super::file::{self, folders};
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::Path;

/// Where the folder of `workspace add <name>` comes from.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    /// An existing directory at `<workspace dir>/<name>`.
    Adopt,
    Clone(String),
    Init(String),
}

/// Scaffolds a project of this type at this (not yet existing) path.
pub type Scaffold<'a> = &'a dyn Fn(&str, &Path) -> Result<()>;

fn base(ws_file: &Path) -> String {
    ws_file
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned())
}

/// Replaces the file through a temp file in its directory, so a failure cannot corrupt it.
fn write_atomically(ws_file: &Path, text: &str) -> Result<()> {
    let tmp = ws_file.with_extension(format!("code-workspace.tmp-{}", std::process::id()));
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, ws_file).with_context(|| format!("replacing {}", ws_file.display()))
}

fn read(ws_file: &Path) -> Result<String> {
    std::fs::read_to_string(ws_file).with_context(|| format!("reading {}", ws_file.display()))
}

/// Creates the folder as `source` says, announcing it; refuses to overwrite anything.
fn provide(
    sub: &Path,
    name: &str,
    source: &Source,
    git: &impl Exec,
    scaffold: Scaffold,
    out: &mut impl Write,
) -> Result<()> {
    match source {
        Source::Adopt if !sub.is_dir() => bail!(
            "workspace add {name}: {} does not exist. Use --clone <url> or --init <type>, or create it first.",
            sub.display()
        ),
        Source::Adopt => {}
        Source::Clone(_) | Source::Init(_) if sub.exists() => {
            let verb = if matches!(source, Source::Clone(_)) {
                "clone"
            } else {
                "scaffold"
            };
            bail!(
                "workspace add {name}: {} already exists; refusing to {verb} over it",
                sub.display()
            )
        }
        Source::Clone(url) => {
            writeln!(out, "==> Cloning {url} into {}", sub.display())?;
            git.run("git", &["clone", url, &sub.to_string_lossy()], None)?;
        }
        Source::Init(kind) => {
            writeln!(
                out,
                "==> Scaffolding new {kind} project at {}",
                sub.display()
            )?;
            scaffold(kind, sub)?;
        }
    }
    Ok(())
}

pub fn add(
    ws_file: &Path,
    name: &str,
    source: &Source,
    git: &impl Exec,
    scaffold: Scaffold,
    out: &mut impl Write,
) -> Result<()> {
    if name.is_empty() {
        bail!("add needs a name.");
    }
    let json = read(ws_file)?;
    folders(&json)?; // a broken workspace stops us before we clone or scaffold anything
    let sub = ws_file.parent().unwrap_or(Path::new(".")).join(name);
    provide(&sub, name, source, git, scaffold, out)?;
    let file = base(ws_file);
    match file::add(&json, name, name)? {
        Some(text) => {
            write_atomically(ws_file, &text)?;
            writeln!(
                out,
                "==> Added folder {{ name: {name}, path: {name} }} to {file}"
            )?;
        }
        None => writeln!(out, "==> {name} already in {file}; no change.")?,
    }
    Ok(())
}

pub fn remove(ws_file: &Path, name: &str, out: &mut impl Write) -> Result<()> {
    let file = base(ws_file);
    let Some(text) = file::remove(&read(ws_file)?, name)? else {
        bail!("{name} not in {file}; nothing to remove.");
    };
    write_atomically(ws_file, &text)?;
    writeln!(
        out,
        "==> Removed folder {name} from {file} (subfolder kept on disk)."
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("edit_tests.rs");
}
