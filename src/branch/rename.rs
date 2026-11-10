//! Checks, then runs, the rename of a repository's branch (see `plan`).
use super::plan::{Opts, Step, plan, validate};
use crate::scaffold::setup::Exec;
use actautoma::forge::Forge;
use anyhow::{Result, anyhow, bail};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

#[derive(Debug)]
pub struct Outcome {
    pub steps: Vec<Step>,
    /// Tracked files that still name the old branch: `file:line:text`.
    pub warnings: Vec<String>,
    pub executed: bool,
    /// The target already exists and the source does not: nothing to do.
    pub already: bool,
}

fn git_lines(exec: &dyn Exec, path: &Path, args: &[&str]) -> Result<Vec<String>> {
    let out = exec
        .run("git", args, Some(path))
        .map_err(|e| anyhow!("git {} failed in {}: {e}", args.join(" "), path.display()))?;
    Ok(out.lines().map(str::to_string).collect())
}

/// `owner/repo` of `origin`, when it has one, after checking its default branch is `from`.
fn remote_slug(
    exec: &dyn Exec,
    forge: &dyn Forge,
    path: &Path,
    from: &str,
) -> Result<Option<String>> {
    if !git_lines(exec, path, &["remote"])?
        .iter()
        .any(|r| r == "origin")
    {
        return Ok(None);
    }
    let repo = forge
        .view_repo()
        .map_err(|e| anyhow!("reading the hosted repository failed: {e}"))?;
    if repo.default_branch != from {
        let default = &repo.default_branch;
        bail!(
            "the default branch on the platform is {default:?}, not {from:?}: nothing was changed"
        );
    }
    Ok(Some(repo.slug))
}

/// Whole-word match, as `git grep -w`: word characters are letters, digits, `_`.
pub fn mentions(line: &str, word: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(word).any(|(i, _)| {
        let before = line[..i].chars().next_back();
        let after = line[i + word.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

fn references(exec: &dyn Exec, path: &Path, word: &str) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for name in git_lines(exec, path, &["ls-files"])? {
        if name.ends_with(".lock") {
            continue; // lock files hold dependency data, never a branch to follow
        }
        let text = match fs::read_to_string(path.join(&name)) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::InvalidData => continue, // binary file: no reference to read
            Err(e) => {
                found.push(format!("{name}: not scanned: {e}"));
                continue;
            }
        };
        for (n, line) in text.lines().enumerate().filter(|(_, l)| mentions(l, word)) {
            found.push(format!("{name}:{}:{}", n + 1, line.trim()));
        }
    }
    Ok(found)
}

fn execute(exec: &dyn Exec, forge: &dyn Forge, path: &Path, steps: &[Step]) -> Result<()> {
    let mut done: Vec<String> = Vec::new();
    for step in steps {
        let outcome = match step {
            Step::Git(args) => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                exec.run("git", &args, Some(path)).map(drop)
            }
            Step::SetDefaultBranch { slug, branch } => forge
                .set_default_branch(slug, branch)
                .map_err(anyhow::Error::from),
        };
        outcome.map_err(|e| {
            let so_far = if done.is_empty() {
                "nothing".to_string()
            } else {
                done.join(", ")
            };
            anyhow!("{} failed: {e}; already done: {so_far}", step.line())
        })?;
        done.push(step.line());
    }
    Ok(())
}

pub fn rename(
    exec: &dyn Exec,
    forge: &dyn Forge,
    path: &Path,
    o: &Opts,
    dry_run: bool,
) -> Result<Outcome> {
    validate(o)?;
    let branches = git_lines(exec, path, &["branch", "--format=%(refname:short)"])?;
    let has = |name: &str| branches.iter().any(|b| b == name);
    let none = |already| Outcome {
        steps: vec![],
        warnings: vec![],
        executed: false,
        already,
    };
    if !has(&o.from) {
        if has(&o.to) {
            return Ok(none(true));
        }
        bail!("no branch named {:?} in {}", o.from, path.display());
    }
    if has(&o.to) {
        bail!(
            "branch {:?} already exists next to {:?}: nothing was changed",
            o.to,
            o.from
        );
    }
    let slug = remote_slug(exec, forge, path, &o.from)?;
    let warnings = references(exec, path, &o.from)?;
    let steps = plan(o, slug.as_deref());
    if !dry_run {
        execute(exec, forge, path, &steps)?;
    }
    Ok(Outcome {
        steps,
        warnings,
        executed: !dry_run,
        already: false,
    })
}

#[cfg(test)]
mod tests {
    include!("rename_tests.rs");
}
