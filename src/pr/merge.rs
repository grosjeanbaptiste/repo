//! `repo pr merge [N] [--yes]` (port of dotfiles `project pr merge`).
use super::after;
use crate::scaffold::setup::Exec;
use actautoma::forge::{Forge, Mergeable};
use anyhow::{Result, anyhow, bail};
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct Opts {
    pub number: Option<u64>,
    pub yes: bool,
}

/// How long to wait for GitHub to compute mergeability.
#[derive(Debug, PartialEq, Eq)]
pub struct Wait {
    pub tries: u32,
    pub secs: u64,
}

pub fn wait_from_env(env: impl Fn(&str) -> Option<String>) -> Result<Wait> {
    fn read<T: std::str::FromStr>(
        env: &impl Fn(&str) -> Option<String>,
        key: &str,
        default: T,
    ) -> Result<T> {
        match env(key) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| anyhow!("{key} must be a non-negative whole number, got {v:?}")),
        }
    }
    Ok(Wait {
        tries: read(&env, "DOTFILES_PR_MERGEABLE_TRIES", 10)?,
        secs: read(&env, "DOTFILES_PR_MERGEABLE_WAIT", 3)?,
    })
}

fn confirmed(opts: &Opts, input: &mut impl BufRead, out: &mut impl Write) -> Result<bool> {
    if opts.yes {
        return Ok(true);
    }
    write!(out, "Merge this pull request? [y/N] ")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?; // end of input leaves it empty: no answer is a no
    Ok(matches!(line.trim(), "y" | "Y" | "yes" | "YES"))
}

#[allow(clippy::too_many_arguments)] // seams: forge/probe/act/sleep/dir/streams, each a distinct port
pub fn run(
    opts: &Opts,
    wait: &Wait,
    forge: &impl Forge,
    probe: &impl Exec,
    act: &impl Exec,
    sleep: &dyn Fn(u64),
    dir: &Path,
    input: &mut impl BufRead,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let mut pr = forge.view_pr(opts.number)?;
    // Platforms compute mergeability lazily and answer UNKNOWN just after a push.
    let mut tried = 0;
    while pr.mergeable == Mergeable::Unknown && tried < wait.tries {
        tried += 1;
        writeln!(
            err,
            "Mergeability of PR #{} not computed yet by GitHub; asking again in {}s ({tried}/{}).",
            pr.number, wait.secs, wait.tries
        )?;
        sleep(wait.secs);
        pr = forge.view_pr(Some(pr.number))?;
    }
    writeln!(
        out,
        "\nPR #{}\n  Title: {}\n  Source: {}\n  Target: {}\n  Author: {}",
        pr.number, pr.title, pr.head, pr.base, pr.author
    )?;
    writeln!(
        out,
        "  Mergeable: {}\n  Review: {}\n  Merge state: {}\n",
        pr.mergeable, pr.review, pr.merge_state
    )?;
    if pr.mergeable != Mergeable::Mergeable {
        writeln!(err, "This pull request is not ready to merge.")?;
        bail!("PR #{} is not mergeable ({})", pr.number, pr.mergeable);
    }
    if !confirmed(opts, input, out)? {
        writeln!(out, "Cancelled.")?;
        return Ok(());
    }
    forge.merge_pr(pr.number)?;
    after::update(probe, act, dir, &pr.base, &pr.head, out, err)
}

#[cfg(test)]
mod tests {
    include!("merge_tests.rs");
}
