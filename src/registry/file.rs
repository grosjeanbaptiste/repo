//! `$DOTFILES_STATE_ROOT/projects.txt`: one `name|path` per line.
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
}

impl Entry {
    /// Case-insensitive like the shell `sort` was; exact name then path break ties.
    fn sort_key(&self) -> (String, String, PathBuf) {
        (
            self.name.to_lowercase(),
            self.name.clone(),
            self.path.clone(),
        )
    }
}

pub fn sorted(entries: &[Entry]) -> Vec<Entry> {
    let mut v = entries.to_vec();
    v.sort_by_key(Entry::sort_key);
    v
}

/// `<state root>/projects.txt`; the root is `DOTFILES_STATE_ROOT`, else `~/machine`.
pub fn path(env: impl Fn(&str) -> Option<String>) -> Result<PathBuf> {
    if let Some(root) = env("DOTFILES_STATE_ROOT").filter(|r| !r.is_empty()) {
        return Ok(PathBuf::from(root).join("projects.txt"));
    }
    let home = env("HOME")
        .filter(|h| !h.is_empty())
        .context("HOME is not set")?;
    Ok(PathBuf::from(home).join("machine/projects.txt"))
}

/// Entries of the registry text, comments and blank lines dropped; a row
/// that is not `name|path` is an error naming its line.
pub fn parse(text: &str) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        match line.split_once('|') {
            Some((name, path)) if !name.trim().is_empty() && !path.trim().is_empty() => {
                entries.push(Entry {
                    name: name.into(),
                    path: path.into(),
                });
            }
            _ => bail!(
                "projects.txt line {}: expected name|path, got {line:?}",
                i + 1
            ),
        }
    }
    Ok(entries)
}

/// A missing file is an empty registry; an unreadable one is an error.
pub fn read(file: &Path) -> Result<Vec<Entry>> {
    match fs::read_to_string(file) {
        Ok(text) => parse(&text).with_context(|| file.display().to_string()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
    }
}

pub fn find(entries: &[Entry], name: &str) -> Option<PathBuf> {
    entries
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.path.clone())
}

/// The `list` report: sorted, `(missing)` flags a path gone from disk.
pub fn list(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in sorted(entries) {
        let flag = if e.path.is_dir() { "" } else { " (missing)" };
        out.push_str(&format!("{:<30} {}{flag}\n", e.name, e.path.display()));
    }
    out
}

#[cfg(test)]
mod tests {
    include!("file_tests.rs");
}
