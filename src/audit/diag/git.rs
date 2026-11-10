//! Checks of the repository itself: submodules, LFS, pre-commit, husky.
use super::{Finding, Host, fail, ok, warn};
use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

pub fn submodules(root: &Path, h: &Host) -> Option<Finding> {
    if !root.join(".gitmodules").is_file() {
        return None;
    }
    let dir = root.to_string_lossy();
    let status = match h.run("git", &["-C", &dir, "submodule", "status"], root) {
        Ok(s) => s,
        Err(e) => {
            return Some(fail(format!(
                "submodules: `git submodule status` failed: {e:#}"
            )));
        }
    };
    let total = status.lines().count();
    let uninit = status.lines().filter(|l| l.starts_with('-')).count();
    Some(if uninit > 0 {
        warn(format!(
            "submodules: {uninit} of {total} uninitialized (run `git submodule update --init --recursive`)"
        ))
    } else {
        ok(format!("submodules: {total} initialized"))
    })
}

pub fn lfs(root: &Path, h: &Host) -> Result<Option<Finding>> {
    let file = root.join(".gitattributes");
    if !file.is_file() || !read(&file)?.contains("filter=lfs") {
        return Ok(None);
    }
    Ok(Some(if (h.has_tool)("git-lfs") {
        ok("git-lfs available for tracked patterns")
    } else {
        warn(
            "git-lfs not installed but .gitattributes uses filter=lfs (suggest: `dotfiles add git-lfs`)",
        )
    }))
}

pub fn precommit(root: &Path, h: &Host) -> Option<Finding> {
    if !root.join(".pre-commit-config.yaml").is_file()
        && !root.join(".pre-commit-config.yml").is_file()
    {
        return None;
    }
    let hook = std::fs::metadata(root.join(".git/hooks/pre-commit"))
        .is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
    let tool = (h.has_tool)("pre-commit");
    Some(match (hook, tool) {
        (true, true) => ok("pre-commit hook installed"),
        (_, true) => {
            warn("pre-commit installed but hook not wired in this repo (run `pre-commit install`)")
        }
        _ => warn(
            ".pre-commit-config.yaml present but pre-commit not installed (suggest: `dotfiles add pre-commit`)",
        ),
    })
}

pub fn husky(root: &Path) -> Result<Option<Finding>> {
    let file = root.join("package.json");
    if !file.is_file() || !read(&file)?.contains("\"husky\"") {
        return Ok(None);
    }
    Ok(Some(if root.join(".husky/_").is_dir() {
        ok("husky hooks installed")
    } else {
        warn(
            "husky declared in package.json but .husky/_ missing (run `npx husky install` or your package manager equivalent)",
        )
    }))
}

#[cfg(test)]
mod tests {
    include!("git_tests.rs");
}
