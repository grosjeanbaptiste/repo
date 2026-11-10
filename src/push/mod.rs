//! `repo push`: push a repository and every repository nested under it.
pub mod confirm;
pub mod report;
pub mod rows;

use crate::scaffold::setup::Exec;
use anyhow::{Context, Result, bail};
use report::{candidate_list, candidates, summary, table};
use rows::{Row, State, collect};
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub struct Opts {
    pub dir: PathBuf,
    pub dry_run: bool,
    pub yes: bool,
}

/// Probes with `probe` (quiet), pushes with `pusher`; the question and the report go to `out`,
/// warnings and push failures to `err`. Fails when any push failed.
pub fn run(
    opts: &Opts,
    probe: &impl Exec,
    pusher: &impl Exec,
    input: &mut impl BufRead,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let mut warnings = Vec::new();
    let rows = collect(probe, &opts.dir, &mut warnings)?;
    for r in &rows {
        if let State::Error(why) = &r.state {
            warnings.push(format!("{}: {why}", r.rel));
        }
    }
    for w in &warnings {
        writeln!(err, "warn: {w}").context("writing a warning")?;
    }
    let target = opts
        .dir
        .canonicalize()
        .with_context(|| format!("{}: no such directory", opts.dir.display()))?;
    writeln!(out, "{}", table(&target, &rows)).context("writing the report")?;
    let todo = candidates(&rows);
    if todo.is_empty() {
        writeln!(
            out,
            "==> nothing to push (no repo has unpushed commits on a clean tree)."
        )
        .context("writing the report")?;
        return Ok(());
    }
    write!(out, "{}", candidate_list(&todo)).context("writing the report")?;
    if opts.dry_run {
        writeln!(out, "\n==> dry-run: nothing pushed.").context("writing the report")?;
        return Ok(());
    }
    if !opts.yes && !confirm::ask(todo.len(), input, out)? {
        writeln!(out, "\n==> aborted: nothing pushed.").context("writing the report")?;
        return Ok(());
    }
    push_all(&todo, pusher, out, err)
}

fn push_all(
    todo: &[(&Row, u64)],
    pusher: &impl Exec,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let (mut pushed, mut failed) = (0, 0);
    for (row, _) in todo {
        writeln!(out, "\n==> git -C {} push", row.rel).context("writing progress")?;
        let dir = row.repo.to_string_lossy();
        match pusher.run("git", &["-C", &dir, "push"], None) {
            Ok(_) => pushed += 1,
            Err(e) => {
                writeln!(err, "    push failed for {}: {e:#}", row.rel)
                    .context("writing a failure")?;
                failed += 1;
            }
        }
    }
    writeln!(out, "\n{}", summary(pushed, todo.len(), failed)).context("writing the summary")?;
    if failed > 0 {
        bail!("{failed} push(es) failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
