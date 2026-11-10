//! `project scan`: refresh the registry from the repositories found on disk.
use super::discover::discover;
use super::file::{Entry, read};
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub added: usize,
    pub unchanged: usize,
    /// Entries whose path no longer exists.
    pub removed: usize,
    /// Non-fatal trouble (unreadable directories, failed `git worktree list`), never dropped silently.
    pub warnings: Vec<String>,
}

impl Summary {
    pub fn message(&self, registry: &Path) -> String {
        format!(
            "✓ Scan complete: {} added, {} unchanged, {} removed (stale paths). Registry: {}",
            self.added,
            self.unchanged,
            self.removed,
            registry.display()
        )
    }
}

fn physical(p: &Path) -> Option<PathBuf> {
    p.canonicalize().ok() // symlinks resolved: git prints physical paths, the registry keeps its spelling
}

/// True when a linked worktree of `repo` (not `repo` itself) is already
/// registered: that worktree stands for the repository.
fn worktree_registered(
    exec: &impl Exec,
    repo: &Path,
    kept: &[Entry],
    warnings: &mut Vec<String>,
) -> bool {
    let Some(main) = physical(repo) else {
        return false;
    };
    let dir = main.to_string_lossy();
    let listing = match exec.run(
        "git",
        &["-C", &dir, "worktree", "list", "--porcelain"],
        None,
    ) {
        Ok(l) => l,
        Err(e) => {
            warnings.push(format!(
                "git worktree list failed in {}; its worktrees are not checked: {e:#}",
                main.display()
            ));
            return false;
        }
    };
    let registered: Vec<PathBuf> = kept.iter().filter_map(|e| physical(&e.path)).collect();
    listing
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(Path::new)
        .filter(|w| *w != main)
        .any(|w| registered.iter().any(|r| r == w))
}

fn write_atomically(registry: &Path, entries: &[Entry]) -> Result<()> {
    let dir = registry.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let mut lines: Vec<String> = entries
        .iter()
        .map(|e| format!("{}|{}", e.name, e.path.display()))
        .collect();
    lines.sort();
    lines.dedup();
    let tmp = dir.join(format!(".projects.txt.tmp-{}", std::process::id()));
    let text = lines.iter().map(|l| format!("{l}\n")).collect::<String>();
    fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, registry).with_context(|| format!("replacing {}", registry.display()))
}

/// Adds new repositories under `roots`, prunes stale entries, keeps the rest,
/// and rewrites `registry` sorted and unique. Nothing is written on error.
pub fn scan(registry: &Path, roots: &[PathBuf], exec: &impl Exec) -> Result<Summary> {
    let mut summary = Summary::default();
    let (live, stale): (Vec<Entry>, Vec<Entry>) =
        read(registry)?.into_iter().partition(|e| e.path.is_dir());
    summary.removed = stale.len();
    let mut kept = live;
    for root in roots {
        let found = discover(root);
        summary.warnings.extend(found.unreadable);
        for repo in found.repos {
            if kept.iter().any(|e| e.path == repo)
                || worktree_registered(exec, &repo, &kept, &mut summary.warnings)
            {
                summary.unchanged += 1;
                continue;
            }
            let name = repo.file_name().map_or_else(
                || repo.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            kept.push(Entry { name, path: repo });
            summary.added += 1;
        }
    }
    write_atomically(registry, &kept)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    include!("scan_tests.rs");
}
