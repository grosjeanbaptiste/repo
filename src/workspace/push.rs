//! `workspace push [--dry-run]`: push every folder that has unpushed commits.
use super::file::folders;
use super::status::{State, classify};
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

#[derive(Default)]
struct Tally {
    pushed: usize,
    skipped: usize,
    failed: usize,
    unchecked: usize,
}

fn push_one(
    name: &str,
    abs: &Path,
    n: u64,
    dry_run: bool,
    pusher: &impl Exec,
    t: &mut Tally,
    out: &mut impl Write,
) -> Result<()> {
    if dry_run {
        writeln!(out, "  + would push {name:<22} ({n} commit(s) ahead)")?;
        t.pushed += 1;
        return Ok(());
    }
    writeln!(out, "==> pushing {name} ({n} commit(s) ahead)...")?;
    match pusher.run("git", &["-C", &abs.to_string_lossy(), "push"], None) {
        Ok(_) => {
            writeln!(out, "    ok")?;
            t.pushed += 1;
        }
        Err(e) => {
            for line in format!("{e:#}").lines() {
                writeln!(out, "    {line}")?;
            }
            t.failed += 1;
        }
    }
    Ok(())
}

/// `probe` and `pusher` must be quiet: a push failure is reported indented under its folder.
/// Fails when any push failed or any folder could not be classified.
pub fn run(
    ws_file: &Path,
    dry_run: bool,
    probe: &impl Exec,
    pusher: &impl Exec,
    out: &mut impl Write,
) -> Result<()> {
    let json = std::fs::read_to_string(ws_file)
        .with_context(|| format!("reading {}", ws_file.display()))?;
    let dir = ws_file.parent().unwrap_or(Path::new("."));
    let (mut seen, mut t) = (HashSet::new(), Tally::default());
    for f in folders(&json)? {
        let joined = dir.join(&f.path);
        // two entries for one directory must not push twice
        let abs = joined.canonicalize().unwrap_or(joined);
        if !seen.insert(abs.clone()) {
            continue;
        }
        match classify(probe, &abs) {
            State::Ahead(n) => push_one(&f.name, &abs, n, dry_run, pusher, &mut t, out)?,
            State::UpToDate => {
                writeln!(out, "  = {:<22} up to date", f.name)?;
                t.skipped += 1;
            }
            State::Error(why) => {
                writeln!(out, "  ? {:<22} unknown status: error: {why}", f.name)?;
                t.unchecked += 1;
            }
            other => {
                writeln!(out, "  ! {:<22} skipped ({other})", f.name)?;
                t.skipped += 1;
            }
        }
    }
    summary(&t, dry_run, out)
}

fn summary(t: &Tally, dry_run: bool, out: &mut impl Write) -> Result<()> {
    writeln!(out)?;
    if dry_run {
        writeln!(
            out,
            "dry-run: {} would push, {} up-to-date/skipped",
            t.pushed, t.skipped
        )?;
    } else {
        write!(out, "{} pushed, {} up-to-date/skipped", t.pushed, t.skipped)?;
        if t.failed > 0 {
            write!(out, ", {} failed", t.failed)?;
        }
        if t.unchecked > 0 {
            write!(out, ", {} unchecked", t.unchecked)?;
        }
        writeln!(out)?;
    }
    let mut problems = Vec::new();
    if t.failed > 0 {
        problems.push(format!("{} push(es) failed", t.failed));
    }
    if t.unchecked > 0 {
        problems.push(format!("{} folder(s) could not be checked", t.unchecked));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        bail!("{}", problems.join(", "))
    }
}

#[cfg(test)]
mod tests {
    include!("push_tests.rs");
}
