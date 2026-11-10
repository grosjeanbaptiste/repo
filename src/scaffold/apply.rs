//! `check` / `apply`: compare or refresh an existing project against the
//! templates. Returns the report lines (same wording as project-scaffold.sh).
use super::init::{content, destination, is_link, link_target, write};
use super::plan::Entry;
use super::render::Vars;
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Check,
    /// Skip existing files.
    Apply,
    Force,
    /// Report what would change, with a unified diff; write nothing.
    Diff,
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn unified_diff(old: &Path, new: &[u8]) -> Result<String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("diff")
        .arg("-u")
        .arg(old)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("diff not found")?;
    let mut stdin = child.stdin.take().context("diff stdin")?;
    stdin.write_all(new).context("feeding diff")?;
    drop(stdin); // close the pipe so diff sees EOF
    let out = child.wait_with_output().context("waiting for diff")?;
    // exit 1 = files differ (expected); 2 = trouble
    if out.status.code().is_none_or(|c| c > 1) {
        anyhow::bail!("diff -u {} failed: {}", old.display(), out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| format!("    {l}\n"))
        .collect())
}

fn symlink_line(entry: &Entry, dst: &Path, rel: &str, mode: Mode) -> Result<String> {
    let want = link_target(entry)?;
    let current = fs::read_link(dst)
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    let same = current.as_deref() == Some(want.as_str());
    Ok(match mode {
        Mode::Check if same => format!("  = ok       {rel}"),
        Mode::Check => match current {
            Some(c) => format!("  ~ drifted  {rel} (symlink target: {c}, expected: {want})"),
            None if exists(dst) => format!("  ~ drifted  {rel} (not a symlink, expected → {want})"),
            None => format!("  - missing  {rel}"),
        },
        _ if same => format!("  = unchanged  {rel}"),
        Mode::Diff if exists(dst) => format!("  ~ would replace with symlink  {rel} -> {want}"),
        Mode::Diff => format!("  + would create symlink  {rel} -> {want}"),
        Mode::Apply if exists(dst) => format!("  ! exists (skipped, use --force)  {rel}"),
        _ => {
            if let Some(dir) = dst.parent() {
                fs::create_dir_all(dir).with_context(|| format!("mkdir {}", dir.display()))?;
            }
            if exists(dst) {
                fs::remove_file(dst).with_context(|| format!("replacing {}", dst.display()))?;
            }
            symlink(&want, dst).with_context(|| format!("linking {}", dst.display()))?;
            format!("  + linked     {rel} -> {want}")
        }
    })
}

fn file_line(entry: &Entry, dst: &Path, rel: &str, mode: Mode, vars: &Vars) -> Result<String> {
    let want = content(entry, vars)?;
    let have = if exists(dst) {
        fs::read(dst).ok()
    } else {
        None
    };
    let same = have.as_deref() == Some(want.as_slice());
    Ok(match mode {
        Mode::Check if have.is_none() && !exists(dst) => format!("  - missing  {rel}"),
        Mode::Check if same => format!("  = ok       {rel}"),
        Mode::Check => format!("  ~ drifted  {rel}"),
        Mode::Diff if !exists(dst) => format!("  + would create  {rel}"),
        _ if same => format!("  = unchanged  {rel}"),
        Mode::Diff => format!(
            "  ~ would update  {rel} (diff below)\n{}",
            unified_diff(dst, &want)?.trim_end()
        ),
        Mode::Apply if exists(dst) => format!("  ! exists (skipped, use --force)  {rel}"),
        _ => {
            if let Some(dir) = dst.parent() {
                fs::create_dir_all(dir).with_context(|| format!("mkdir {}", dir.display()))?;
            }
            write(entry, dst, vars)?;
            format!("  + wrote      {rel}")
        }
    })
}

pub fn sync(target: &Path, plan: &[Entry], vars: &Vars, mode: Mode) -> Result<Vec<String>> {
    plan.iter()
        .map(|entry| {
            let dst = destination(target, entry, vars);
            let rel = dst
                .strip_prefix(target)
                .unwrap_or(&dst)
                .to_string_lossy()
                .into_owned();
            if is_link(entry)? {
                symlink_line(entry, &dst, &rel, mode)
            } else {
                file_line(entry, &dst, &rel, mode, vars)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    include!("apply_tests.rs");
}
