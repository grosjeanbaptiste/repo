//! The `Git` section of `doctor`: branch, upstream, working tree, last commit.
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result};
use std::path::Path;

fn row(key: &str, value: &str) -> String {
    format!("    {key:<22} {value}")
}

fn count(exec: &dyn Exec, root: &Path, range: &str, what: &str) -> Result<u64> {
    let raw = exec
        .run("git", &["rev-list", "--count", range], Some(root))
        .with_context(|| format!("counting commits {what}"))?;
    raw.trim()
        .parse()
        .with_context(|| format!("counting commits {what}: git answered {raw:?}"))
}

fn tracking(exec: &dyn Exec, root: &Path) -> Result<String> {
    // No upstream is a normal answer, not a failure.
    let args = ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"];
    let Ok(up) = exec.run("git", &args, Some(root)) else {
        return Ok(row("Tracking:", "(no upstream)"));
    };
    let up = up.trim();
    let ahead = count(exec, root, &format!("{up}..HEAD"), "ahead")?;
    let behind = count(exec, root, &format!("HEAD..{up}"), "behind")?;
    Ok(row(
        &format!("vs {up}:"),
        &format!("{ahead} ahead, {behind} behind"),
    ))
}

pub fn lines(exec: &dyn Exec, root: &Path) -> Result<Vec<String>> {
    if !root.join(".git").is_dir() {
        return Ok(vec!["    (not a git repo)".into()]);
    }
    let git = |args: &[&str]| exec.run("git", args, Some(root));
    // An unborn branch has no name to give.
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .map_or("(detached)".into(), |b| b.trim().to_string());
    let status = git(&["--no-optional-locks", "status", "--porcelain"])
        .context("reading the working tree")?;
    let dirty = status.lines().filter(|l| !l.trim().is_empty()).count();
    // A repository without commit has no last commit: that is the answer.
    let last = git(&["log", "-1", "--format=%h %s"])
        .map_or("(no commits)".into(), |c| c.trim().to_string());
    let age = git(&["log", "-1", "--format=%ar"]).map_or("?".into(), |a| a.trim().to_string());
    Ok(vec![
        row("Branch:", &branch),
        tracking(exec, root)?,
        row("Working tree:", &format!("{dirty} uncommitted change(s)")),
        row("Last commit:", &format!("{last} — {age}")),
    ])
}

#[cfg(test)]
mod tests {
    include!("git_tests.rs");
}
