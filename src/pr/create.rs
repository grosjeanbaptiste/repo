//! `repo pr create [--base B] [--title T] [--body D]` (port of dotfiles `project pr create`,
//! with flags in place of the fzf base picker; title and description are asked when not given).
use crate::scaffold::setup::Exec;
use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Opts {
    pub base: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
}

/// Base branches a PR may target: `master|main|develop|dev|staging|release/…`, sorted, unique.
pub fn candidates(remote_branches: &str) -> Vec<String> {
    const BASES: [&str; 6] = ["master", "main", "develop", "dev", "staging", "release/"];
    let found: BTreeSet<String> = remote_branches
        .lines()
        .map(|l| l.strip_prefix("origin/").unwrap_or(l))
        .filter(|l| BASES.iter().any(|b| l.starts_with(b)))
        .map(String::from)
        .collect();
    found.into_iter().collect()
}

fn base_branch(opts: &Opts, probe: &impl Exec, dir: &Path, out: &mut impl Write) -> Result<String> {
    if let Some(b) = &opts.base {
        return Ok(b.clone());
    }
    let raw = probe.run(
        "git",
        &["branch", "-r", "--format=%(refname:short)"],
        Some(dir),
    )?;
    let found = candidates(&raw);
    let [only] = &found[..] else {
        if found.is_empty() {
            bail!(
                "no base branch found (master, main, develop, dev, staging, release/…): pass --base"
            );
        }
        bail!("several base branches: {}: pass --base", found.join(", "));
    };
    writeln!(out, "Selected base branch: {only}\n")?;
    Ok(only.clone())
}

fn title(
    opts: &Opts,
    probe: &impl Exec,
    dir: &Path,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Result<String> {
    if let Some(t) = &opts.title {
        return Ok(t.clone());
    }
    write!(
        out,
        "PR title (leave empty to use the last commit message): "
    )?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    let typed = line.trim_end_matches(['\n', '\r']);
    if !typed.is_empty() {
        return Ok(typed.to_string());
    }
    let log = probe.run("git", &["log", "-1", "--pretty=%B"], Some(dir))?;
    let subject = log.lines().next().unwrap_or_default().to_string();
    writeln!(out, "Using commit message: {subject}")?;
    Ok(subject)
}

fn body(opts: &Opts, input: &mut impl BufRead, out: &mut impl Write) -> Result<String> {
    if let Some(b) = &opts.body {
        return Ok(b.clone());
    }
    writeln!(
        out,
        "PR description (press Ctrl-D to finish, empty allowed):"
    )?;
    let mut text = String::new();
    for line in input.lines() {
        text.push_str(&line?);
        text.push('\n');
    }
    Ok(text)
}

pub fn run(
    opts: &Opts,
    probe: &impl Exec,
    act: &impl Exec,
    dir: &Path,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Result<()> {
    let head = probe.run("git", &["branch", "--show-current"], Some(dir))?;
    let head = head.trim();
    if head.is_empty() {
        bail!("Could not determine current branch.");
    }
    if head == "main" || head == "master" {
        bail!("You are already on {head}. Create a feature branch first.");
    }
    writeln!(out, "Current branch: {head}\n")?;
    let base = base_branch(opts, probe, dir, out)?;
    let title = title(opts, probe, dir, input, out)?;
    let body = body(opts, input, out)?;
    writeln!(out, "\nDiff against {base}:")?;
    let range = format!("{base}...{head}");
    let stat = probe.run("git", &["diff", "--stat", &range], Some(dir))?;
    if stat.trim().is_empty() {
        bail!("No changes detected between {base} and {head}.");
    }
    write!(out, "{stat}")?;
    writeln!(out, "\nCreating pull request...")?;
    let args = [
        "pr", "create", "--base", &base, "--head", head, "--title", &title, "--body", &body,
    ];
    write!(out, "{}", act.run("gh", &args, Some(dir))?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("create_tests.rs");
}
