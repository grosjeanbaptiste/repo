//! One row per repository: where it is, and whether it can be pushed.
use crate::registry::discover::discover_all;
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum State {
    Dirty,
    NoUpstream,
    UpToDate,
    Ahead(u64),
    /// git failed in an unexpected way: never pushed, reported.
    Error(String),
}

impl State {
    pub fn label(&self) -> String {
        match self {
            Self::Dirty => "dirty".into(),
            Self::NoUpstream => "no-upstream".into(),
            Self::UpToDate => "up-to-date".into(),
            Self::Ahead(n) => format!("ahead:{n}"),
            Self::Error(_) => "error".into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Row {
    pub repo: PathBuf,
    /// Path relative to the scan root (`.` for the root itself).
    pub rel: String,
    pub branch: String,
    pub remote: String,
    pub state: State,
}

fn git(exec: &impl Exec, repo: &Path, args: &[&str]) -> Result<String> {
    let dir = repo.to_string_lossy();
    let mut full = vec!["--no-optional-locks", "-C", &dir];
    full.extend_from_slice(args);
    exec.run("git", &full, None)
}

fn state(exec: &impl Exec, repo: &Path) -> Result<State> {
    if !git(exec, repo, &["status", "--porcelain=v1"])?
        .trim()
        .is_empty()
    {
        return Ok(State::Dirty);
    }
    let Ok(upstream) = git(exec, repo, &["rev-parse", "--abbrev-ref", "@{upstream}"]) else {
        return Ok(State::NoUpstream); // no upstream is a normal state, not a failure
    };
    let range = format!("{}..HEAD", upstream.trim());
    let out = git(exec, repo, &["rev-list", "--count", &range])?;
    let ahead: u64 = out
        .trim()
        .parse()
        .with_context(|| format!("git rev-list --count {range}: {out:?}"))?;
    Ok(if ahead == 0 {
        State::UpToDate
    } else {
        State::Ahead(ahead)
    })
}

pub fn classify(exec: &impl Exec, repo: &Path, rel: &str) -> Result<Row> {
    let branch = git(exec, repo, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map_or_else(|_| "detached".to_string(), |b| b.trim().to_string()); // no symbolic ref = detached HEAD
    let remote = git(exec, repo, &["remote", "get-url", "origin"])
        .map_or_else(|_| "(no origin)".to_string(), |r| r.trim().to_string()); // no origin is a normal state
    let state = state(exec, repo).unwrap_or_else(|e| State::Error(format!("{e:#}")));
    Ok(Row {
        repo: repo.to_path_buf(),
        rel: rel.to_string(),
        branch,
        remote,
        state,
    })
}

fn relative(target: &Path, repo: &Path) -> String {
    match repo.strip_prefix(target) {
        Ok(r) if r.as_os_str().is_empty() => ".".to_string(),
        Ok(r) => r.display().to_string(),
        Err(_) => repo.display().to_string(), // the enclosing repository lies above the root
    }
}

/// Repositories at and under `target`, the enclosing repository first, outer before inner.
pub fn collect(exec: &impl Exec, target: &Path, warnings: &mut Vec<String>) -> Result<Vec<Row>> {
    let target = target
        .canonicalize()
        .with_context(|| format!("{}: no such directory", target.display()))?;
    let found = discover_all(&target);
    warnings.extend(found.unreadable);
    let mut repos = found.repos;
    if let Ok(outer) = git(exec, &target, &["rev-parse", "--show-toplevel"]) {
        let outer = PathBuf::from(outer.trim());
        if !repos.contains(&outer) {
            repos.push(outer);
        }
    } // not inside a repository: nothing enclosing
    if repos.is_empty() {
        bail!("No git repos found under {}", target.display());
    }
    repos.sort_by_key(|r| (r.to_string_lossy().chars().count(), r.clone()));
    repos
        .iter()
        .map(|r| classify(exec, r, &relative(&target, r)))
        .collect()
}

#[cfg(test)]
mod tests {
    include!("rows_tests.rs");
}
