//! `setup <type> <name>`: mkdir, `git init -b main`, init, commit, and unless
//! `no_push` a private GitHub repository pushed with `gh`. Every external
//! command goes through `Exec` so the sequence is testable.
use super::{init::init, plan::plan, render::Vars};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

pub trait Exec {
    /// Runs `prog args` (in `cwd` when given); stdout on success, an error naming the command otherwise.
    fn run(&self, prog: &str, args: &[&str], cwd: Option<&Path>) -> Result<String>;
}

pub struct Setup<'a> {
    pub templates: &'a Path,
    pub parent: &'a Path,
    pub kind: &'a str,
    pub name: &'a str,
    pub no_push: bool,
    pub year: String,
}

fn config(exec: &impl Exec, key: &str, target: &Path) -> Option<String> {
    let v = exec.run("git", &["config", key], Some(target)).ok()?; // unset key is normal: caller falls back
    Some(v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn vars(exec: &impl Exec, name: &str, year: &str, target: &Path) -> Vars {
    let user = config(exec, "user.name", target)
        .or_else(|| std::env::var("USER").ok()) // same fallback as project-scaffold.sh
        .unwrap_or_default();
    let email = config(exec, "user.email", target).unwrap_or_default();
    [
        ("project_name", name),
        ("user", &user),
        ("email", &email),
        ("year", year),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn check(s: &Setup) -> Result<PathBuf> {
    if s.kind.is_empty() || s.name.is_empty() {
        bail!("setup needs <type> and <name>");
    }
    plan(s.templates, s.kind)?; // Unknown project type
    if !s.parent.is_dir() {
        bail!("Parent directory does not exist: {}", s.parent.display());
    }
    let target = s.parent.join(s.name);
    if fs::symlink_metadata(&target).is_ok() {
        bail!("Target already exists: {}", target.display());
    }
    Ok(target)
}

/// Returns the created directory.
pub fn setup(s: &Setup, exec: &impl Exec) -> Result<PathBuf> {
    let target = check(s)?;
    let plan = plan(s.templates, s.kind)?;
    // Fail before creating anything when a needed tool is missing.
    exec.run("git", &["--version"], None)?;
    if !s.no_push {
        exec.run("gh", &["--version"], None)?;
    }
    fs::create_dir_all(&target).with_context(|| format!("mkdir {}", target.display()))?;
    exec.run("git", &["init", "-q", "-b", "main"], Some(&target))?;
    init(&target, &plan, &vars(exec, s.name, &s.year, &target))?;
    exec.run("git", &["add", "-A"], Some(&target))?;
    let msg = format!("initial scaffold for {} ({})", s.name, s.kind);
    exec.run("git", &["commit", "-q", "-m", &msg], Some(&target))?;
    if !s.no_push {
        let dir = target.to_string_lossy();
        exec.run(
            "gh",
            &[
                "repo",
                "create",
                s.name,
                "--private",
                "--source",
                &dir,
                "--push",
            ],
            None,
        )?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    include!("setup_tests.rs");
}
