//! `project status`: branch / ahead / behind / dirty / remote per project.
use super::file::{Entry, sorted};
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result, bail};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct Status {
    pub branch: String,
    /// `None` when the branch has no upstream.
    pub ahead: Option<u64>,
    pub behind: Option<u64>,
    pub dirty: usize,
    /// `none` when there is no `origin`.
    pub remote: String,
}

fn git(exec: &impl Exec, path: &Path, args: &[&str]) -> Result<String> {
    let dir = path.to_string_lossy();
    let mut full = vec!["--no-optional-locks", "-C", &dir];
    full.extend_from_slice(args);
    exec.run("git", &full, None)
}

fn count(exec: &impl Exec, path: &Path, range: &str) -> Result<u64> {
    let out = git(exec, path, &["rev-list", "--count", range])?;
    out.trim()
        .parse()
        .with_context(|| format!("git rev-list --count {range}: {out:?}"))
}

/// `Ok(None)` when `path` is not a git repository.
pub fn git_status(exec: &impl Exec, path: &Path) -> Result<Option<Status>> {
    if git(exec, path, &["rev-parse", "--git-dir"]).is_err() {
        exec.run("git", &["--version"], None)?; // git itself missing is an error, not "not a repository"
        return Ok(None);
    }
    let branch = git(exec, path, &["symbolic-ref", "--short", "-q", "HEAD"])
        .map_or_else(|_| "HEAD".to_string(), |b| b.trim().to_string()); // detached HEAD has no symbolic ref
    let remote = git(exec, path, &["remote", "get-url", "origin"])
        .map_or_else(|_| "none".to_string(), |r| r.trim().to_string()); // no origin is a normal state
    let bare = git(exec, path, &["rev-parse", "--is-bare-repository"])?.trim() == "true";
    let dirty = if bare {
        0 // no working tree, nothing to be dirty
    } else {
        git(exec, path, &["status", "--porcelain"])?
            .lines()
            .filter(|l| !l.is_empty())
            .count()
    };
    let has_upstream = git(exec, path, &["rev-parse", "--abbrev-ref", "@{upstream}"]).is_ok(); // no upstream is normal
    let (ahead, behind) = if has_upstream {
        (
            Some(count(exec, path, "@{upstream}..HEAD")?),
            Some(count(exec, path, "HEAD..@{upstream}")?),
        )
    } else {
        (None, None)
    };
    Ok(Some(Status {
        branch,
        ahead,
        behind,
        dirty,
        remote,
    }))
}

fn cell(n: Option<u64>) -> String {
    n.map_or_else(|| "-".to_string(), |n| n.to_string())
}

/// The report of `project status NAME`; `Err` text when not a repository.
pub fn one(exec: &impl Exec, name: &str, path: &Path) -> Result<String> {
    let Some(s) = git_status(exec, path)? else {
        bail!(
            "Project: {name}\nPath:    {}\nNot a git repository (or path missing).",
            path.display()
        );
    };
    Ok(format!(
        "Project: {name}\nPath:    {}\nBranch:  {}\nAhead:   {}\nBehind:  {}\nDirty:   {} file(s)\nRemote:  {}\n",
        path.display(),
        s.branch,
        cell(s.ahead),
        cell(s.behind),
        s.dirty,
        s.remote
    ))
}

/// The table of `project status`: registered projects that exist and are repositories; a repo that cannot be read is a warning.
pub fn all(exec: &impl Exec, entries: &[Entry], warnings: &mut Vec<String>) -> Result<String> {
    let mut out = format!(
        "{:<30} {:<16} {:>6} {:>6} {:>6}  {}\n",
        "NAME", "BRANCH", "AHEAD", "BEHIND", "DIRTY", "REMOTE"
    );
    for e in sorted(entries).iter().filter(|e| e.path.is_dir()) {
        match git_status(exec, &e.path) {
            Ok(Some(s)) => out.push_str(&format!(
                "{:<30} {:<16} {:>6} {:>6} {:>6}  {}\n",
                e.name,
                s.branch,
                cell(s.ahead),
                cell(s.behind),
                s.dirty,
                s.remote
            )),
            Ok(None) => {} // not a repository any more: not part of the table
            Err(err) => warnings.push(format!("{} ({}): {err:#}", e.name, e.path.display())),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    include!("status_tests.rs");
}
