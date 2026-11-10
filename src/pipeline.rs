//! `repo new`: choose a free name, then scaffold. Both seams are injected so the
//! whole pipeline is testable: GitHub (`RepoNames`) and `dotfiles` (`Scaffold`).
use crate::naming::{self, Query};
use actit::ports::RepoNames;
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub enum Source {
    /// The caller chose the name; it must still be free.
    Named(String),
    /// actname proposes, the first free candidate wins.
    Generated(Query),
}

pub struct NewOpts {
    pub kind: String,
    pub source: Source,
    pub root: PathBuf,
    pub no_push: bool,
    pub dry_run: bool,
}

/// Creates the directory, scaffolds it, commits, and pushes unless `no_push`.
pub trait Scaffold {
    fn setup(&self, kind: &str, name: &str, root: &Path, no_push: bool) -> Result<()>;
}

/// Legacy path: `dotfiles project setup`.
pub struct Dotfiles;

impl Scaffold for Dotfiles {
    fn setup(&self, kind: &str, name: &str, root: &Path, no_push: bool) -> Result<()> {
        crate::delegate::project(&crate::delegate::setup_args(kind, name, root, no_push))
    }
}

fn choose(source: &Source, root: &Path, names: &impl RepoNames) -> Result<String> {
    match source {
        Source::Named(name) if naming::taken(root, name, names)? => {
            bail!("{name} is taken (directory or GitHub repository)")
        }
        Source::Named(name) => Ok(name.clone()),
        Source::Generated(query) => naming::first_free(&naming::candidates(query)?, |n| {
            naming::taken(root, n, names)
        }),
    }
}

/// Returns the name that was (or, with `dry_run`, would be) created.
pub fn run(opts: &NewOpts, names: &impl RepoNames, scaffold: &impl Scaffold) -> Result<String> {
    let name = choose(&opts.source, &opts.root, names)?;
    if !opts.dry_run {
        scaffold.setup(&opts.kind, &name, &opts.root, opts.no_push)?;
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    include!("pipeline_tests.rs");
}
