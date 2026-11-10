//! `repo list|scan|status`: the registry commands, wired to the real process and environment.
use super::file::{find, list, path, read};
use super::{scan::scan, status};
use crate::scaffold::native::Quiet;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

fn registry() -> Result<PathBuf> {
    path(|k| std::env::var(k).ok())
}

pub fn run_list() -> Result<()> {
    let entries = read(&registry()?)?;
    if entries.is_empty() {
        eprintln!("No projects registered yet. Run: repo scan");
    }
    print!("{}", list(&entries));
    Ok(())
}

pub fn run_scan(roots: &[String]) -> Result<()> {
    let registry = registry()?;
    let roots: Vec<PathBuf> = if roots.is_empty() {
        let home = std::env::var("HOME")
            .ok()
            .filter(|h| !h.is_empty())
            .context("HOME is not set")?;
        vec![PathBuf::from(home)]
    } else {
        roots.iter().map(PathBuf::from).collect()
    };
    let summary = scan(&registry, &roots, &Quiet)?;
    for w in &summary.warnings {
        eprintln!("warn: {w}");
    }
    println!("{}", summary.message(&registry));
    Ok(())
}

pub fn run_status(name: Option<&str>) -> Result<()> {
    let entries = read(&registry()?)?;
    match name {
        None => {
            let mut warnings = Vec::new();
            print!("{}", status::all(&Quiet, &entries, &mut warnings)?);
            warnings.iter().for_each(|w| eprintln!("warn: {w}"));
        }
        Some(name) => {
            let Some(path) = find(&entries, name) else {
                bail!("Unknown project: {name}");
            };
            print!("{}", status::one(&Quiet, name, &path)?);
        }
    }
    Ok(())
}
