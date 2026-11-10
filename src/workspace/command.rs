//! The real ports of `repo workspace`: the file lookup, git, stdout.
use super::edit::{self, Source};
use super::{push, show};
use crate::scaffold::native::{Process, Quiet, templates_root, year_now};
use crate::scaffold::setup::{Exec, vars};
use crate::scaffold::{init::init, plan::plan};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// `DOTFILES_WORKSPACE_FILE`, else the first `*.code-workspace` here or above.
fn workspace_file() -> Result<PathBuf> {
    if let Some(f) = std::env::var_os("DOTFILES_WORKSPACE_FILE").filter(|f| !f.is_empty()) {
        return Ok(f.into());
    }
    let here = std::env::current_dir().context("reading the current directory")?;
    match super::file::find(&here)? {
        Some(f) => Ok(f),
        None => bail!(
            "No *.code-workspace file in {} or any ancestor.\nPass DOTFILES_WORKSPACE_FILE=<path> or cd into a workspace.",
            here.display()
        ),
    }
}

/// Stops with a clear message before any folder is probed with a git that is not there.
fn need_git() -> Result<()> {
    Quiet.run("git", &["--version"], None).map(drop)
}

pub fn run_list() -> Result<()> {
    show::list(
        &workspace_file()?,
        std::env::var("HOME").ok().as_deref(),
        &mut std::io::stdout().lock(),
    )
}

pub fn run_status() -> Result<()> {
    need_git()?;
    show::status(&workspace_file()?, &Quiet, &mut std::io::stdout().lock())
}

pub fn run_push(dry_run: bool) -> Result<()> {
    need_git()?;
    push::run(
        &workspace_file()?,
        dry_run,
        &Quiet,
        &Quiet,
        &mut std::io::stdout().lock(),
    )
}

pub fn run_remove(name: &str) -> Result<()> {
    edit::remove(&workspace_file()?, name, &mut std::io::stdout().lock())
}

pub fn run_add(name: &str, source: &Source) -> Result<()> {
    let ws = workspace_file()?;
    let templates = templates_root(|k| std::env::var(k).ok())?;
    let dir = ws.parent().unwrap_or(Path::new(".")).to_path_buf();
    let scaffold = |kind: &str, target: &Path| -> Result<()> {
        let year = year_now()?;
        let label = target
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        // git identity is read next to the workspace: the new folder does not exist yet
        init(
            target,
            &plan(&templates, kind)?,
            &vars(&Process, &label, &year, &dir),
        )
    };
    edit::add(
        &ws,
        name,
        source,
        &Process,
        &scaffold,
        &mut std::io::stdout().lock(),
    )
}
