//! Find git repositories under a root (`find … -prune` of dotfiles, in Rust).
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// Working-tree roots (a directory holding a `.git` directory), sorted.
    pub repos: Vec<PathBuf>,
    /// Directories that could not be read: reported, never silently dropped.
    pub unreadable: Vec<String>,
}

/// Directories never descended into: vendored dependencies, build output and
/// tool/system dirs that carry nested `.git` directories of their own.
pub const PRUNED: [&str; 30] = [
    "build",
    "Pods",
    "SourcePackages",
    "node_modules",
    ".dart_tool",
    "DerivedData",
    "vendor",
    ".venv",
    "venv",
    "Library",
    ".cache",
    ".Trash",
    ".cargo",
    ".rustup",
    ".nvm",
    ".pyenv",
    ".rbenv",
    ".npm",
    ".gem",
    ".docker",
    ".colima",
    ".vscode",
    ".cocoapods",
    ".gradle",
    ".m2",
    ".sdkman",
    ".oh-my-zsh",
    ".local",
    ".claude",
    ".cursor",
];

pub fn discover(root: &Path) -> Found {
    discover_pruning(root, &PRUNED)
}

/// Every repository under `root`, vendored directories included (`project push`).
pub fn discover_all(root: &Path) -> Found {
    discover_pruning(root, &[])
}

fn discover_pruning(root: &Path, pruned: &[&str]) -> Found {
    let mut found = Found::default();
    if !root.is_dir() {
        return found;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                found.unreadable.push(format!("{}: {e}", dir.display()));
                continue;
            }
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            // file_type() does not follow symlinks, like `find` without -L
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name();
            if name == ".git" {
                found.repos.push(dir.clone());
            } else if !pruned.iter().any(|p| name == *p) {
                stack.push(path);
            }
        }
    }
    found.repos.sort();
    found.repos.dedup();
    found
}

#[cfg(test)]
mod tests {
    include!("discover_tests.rs");
}
