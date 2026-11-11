//! The `Quality signals` section of `doctor`: TODO/FIXME files, files touched this week.
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const EXTENSIONS: [&str; 8] = ["py", "js", "ts", "tsx", "rs", "go", "sh", "dart"];
const SKIPPED: [&str; 4] = ["node_modules", ".git", "build", "target"];

fn count_in(dir: &Path, word: &str) -> Result<usize> {
    let mut n = 0;
    for entry in fs::read_dir(dir).with_context(|| format!("cannot search {}", dir.display()))? {
        let entry = entry.with_context(|| format!("cannot search {}", dir.display()))?;
        let (path, kind) = (entry.path(), entry.file_type()?);
        let name = entry.file_name();
        if kind.is_dir() && !SKIPPED.iter().any(|s| name == *s) {
            n += count_in(&path, word)?;
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|e| EXTENSIONS.iter().any(|x| e == *x))
        {
            let bytes =
                fs::read(&path).with_context(|| format!("cannot search {}", path.display()))?;
            // Like `grep -I`: a file with a NUL byte is binary, not source.
            let hit = !bytes.contains(&0) && String::from_utf8_lossy(&bytes).contains(word);
            n += usize::from(hit);
        }
    }
    Ok(n)
}

/// How many source files under `root` carry `word`; symlinks are not followed.
pub fn files_with(root: &Path, word: &str) -> Result<usize> {
    count_in(root, word)
}

fn touched(exec: &dyn Exec, root: &Path) -> Result<usize> {
    let git = |args: &[&str]| exec.run("git", args, Some(root));
    // Without a commit nothing was touched; with one, a log failure is a failure.
    if git(&["rev-parse", "--verify", "-q", "HEAD"]).is_err() {
        return Ok(0);
    }
    let log = git(&[
        "log",
        "--since=7 days ago",
        "--name-only",
        "--pretty=format:",
    ])
    .context("reading the last week of history")?;
    Ok(log
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<HashSet<_>>()
        .len())
}

pub fn lines(exec: &dyn Exec, root: &Path) -> Result<Vec<String>> {
    let row = |k: &str, v: String| format!("    {k:<22} {v}");
    let mut out = vec![
        row("Files with TODO:", files_with(root, "TODO")?.to_string()),
        row("Files with FIXME:", files_with(root, "FIXME")?.to_string()),
    ];
    if root.join(".git").is_dir() {
        out.push(row(
            "Touched (last 7d):",
            format!("{} file(s)", touched(exec, root)?),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    include!("quality_tests.rs");
}
