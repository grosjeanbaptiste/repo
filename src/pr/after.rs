//! After `gh pr merge`: bring the local branches up to date. The PR is merged
//! by then, so every failing step is reported and the call fails, but none
//! hides that the merge itself succeeded.
use crate::scaffold::setup::Exec;
use anyhow::{Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};

/// `dir` is a checkout of the repository; `probe` reads quietly, `act` changes things.
pub fn update(
    probe: &impl Exec,
    act: &impl Exec,
    dir: &Path,
    base: &str,
    head: &str,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    writeln!(out, "\nUpdating local branches...")?;
    let git = |args: &[&str]| act.run("git", args, Some(dir));
    if git(&["fetch", "-p"]).is_err() {
        writeln!(err, "warning: PR merged; git fetch failed.")?;
        bail!("git fetch failed after the merge");
    }
    let Ok(holder) = holder(probe, dir, base) else {
        writeln!(
            err,
            "warning: PR merged; the worktrees could not be listed, {base} was not updated."
        )?;
        bail!("worktrees could not be listed after the merge");
    };
    let mut ok = match holder {
        None => advance_here(&git, base, err)?,
        Some(path) => advance_there(probe, act, dir, &path, base, err)?,
    };
    ok &= delete_head(probe, &git, dir, head, err)?;
    if ok {
        Ok(())
    } else {
        bail!("PR merged, but the local update is incomplete")
    }
}

/// Another worktree (not `dir`'s own) that has `branch` checked out.
fn holder(probe: &impl Exec, dir: &Path, branch: &str) -> Result<Option<PathBuf>> {
    let here = probe.run("git", &["rev-parse", "--show-toplevel"], Some(dir))?;
    let list = probe.run("git", &["worktree", "list", "--porcelain"], Some(dir))?;
    let (wanted, mut path) = (format!("branch refs/heads/{branch}"), "");
    for line in list.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = p;
        } else if line == wanted && Path::new(path) != Path::new(here.trim()) {
            return Ok(Some(PathBuf::from(path)));
        }
    }
    Ok(None)
}

fn advance_here(
    git: &dyn Fn(&[&str]) -> Result<String>,
    base: &str,
    err: &mut impl Write,
) -> Result<bool> {
    if git(&["checkout", base])
        .and_then(|_| git(&["pull"]))
        .is_ok()
    {
        return Ok(true);
    }
    writeln!(err, "warning: PR merged; {base} was not updated.")?;
    Ok(false)
}

fn advance_there(
    probe: &impl Exec,
    act: &impl Exec,
    dir: &Path,
    holder: &Path,
    base: &str,
    err: &mut impl Write,
) -> Result<bool> {
    let h = holder.display().to_string();
    let remote = format!("origin/{base}");
    let mut ok = true;
    if !probe
        .run(
            "git",
            &["--no-optional-locks", "-C", &h, "status", "--porcelain"],
            None,
        )?
        .trim()
        .is_empty()
    {
        writeln!(
            err,
            "warning: PR merged; {base} is not updated: its worktree {h} has local changes."
        )?;
        ok = false;
    } else if act
        .run("git", &["-C", &h, "merge", "--ff-only", &remote], None)
        .is_err()
    {
        writeln!(
            err,
            "warning: PR merged; {base} could not be fast-forwarded in {h}."
        )?;
        ok = false;
    }
    Ok(ok
        & act
            .run("git", &["checkout", "--detach", &remote], Some(dir))
            .is_ok())
}

/// `gh pr merge --delete-branch` may have deleted it already: only a branch that
/// is still there and refuses deletion is a failure.
fn delete_head(
    probe: &impl Exec,
    git: &dyn Fn(&[&str]) -> Result<String>,
    dir: &Path,
    head: &str,
    err: &mut impl Write,
) -> Result<bool> {
    let refname = format!("refs/heads/{head}");
    // an Err from show-ref is its answer "no such branch", not a failure
    if probe
        .run(
            "git",
            &["show-ref", "--verify", "--quiet", &refname],
            Some(dir),
        )
        .is_err()
    {
        return Ok(true);
    }
    if git(&["branch", "-d", head]).is_ok() {
        return Ok(true);
    }
    writeln!(
        err,
        "warning: PR merged; local branch {head} was not deleted."
    )?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    include!("after_tests.rs");
}
