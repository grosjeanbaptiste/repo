//! `repo rename-branch`: wired to the real `git` and `gh`.
use super::plan::Opts;
use super::rename::rename;
use crate::scaffold::native::Process;
use anyhow::Result;
use std::path::Path;

/// Warnings shown before "and N more": a word like `master` can be in prose too.
const SHOWN: usize = 20;

pub fn run(path: &str, from: &str, to: &str, dry_run: bool, delete_old: bool) -> Result<()> {
    let opts = Opts {
        from: from.into(),
        to: to.into(),
        delete_old,
    };
    let out = rename(&Process, Path::new(path), &opts, dry_run)?;
    if out.already {
        println!("{to} already exists and {from} does not: nothing to do");
        return Ok(());
    }
    for w in out.warnings.iter().take(SHOWN) {
        eprintln!("warn: still names {from}: {w}");
    }
    if out.warnings.len() > SHOWN {
        eprintln!("warn: ...and {} more", out.warnings.len() - SHOWN);
    }
    let verb = if out.executed { "ran" } else { "would run" };
    out.steps
        .iter()
        .for_each(|s| println!("{verb}: {}", s.line()));
    if out.steps.len() > 1 && !delete_old {
        eprintln!("kept origin/{from}; remove it with: git push origin --delete {from}");
    }
    Ok(())
}
