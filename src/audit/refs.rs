//! Every `(variable, file)` a project tree references: env templates, workflows, compose files.
use super::parse::{compose_names, env_names, workflow_names};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Turns the text of one file into the variable names it references.
type Parser = fn(&str) -> Vec<String>;

const ENV_TEMPLATES: [&str; 4] = [".env.example", ".env.template", ".env.sample", ".env.dist"];

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ref {
    pub var: String,
    /// Path of the file relative to the audited root.
    pub source: String,
}

/// Regular files below `dir` (symlinks are neither followed nor listed), with their depth
/// below the walk root (children of the root are at depth 1, as in `find`).
fn walk(
    dir: &Path,
    depth: usize,
    max: usize,
    files: &mut Vec<(PathBuf, usize)>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if depth == 1 => {
            return Err(e).with_context(|| format!("reading {}", dir.display()));
        }
        Err(e) => {
            warnings.push(format!("cannot read {}: {e}", dir.display()));
            return Ok(());
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("reading {}", dir.display()))?;
        let kind = entry
            .file_type()
            .with_context(|| format!("reading {}", entry.path().display()))?;
        if kind.is_file() {
            files.push((entry.path(), depth));
        } else if kind.is_dir() && depth < max {
            walk(&entry.path(), depth + 1, max, files, warnings)?;
        }
    }
    Ok(())
}

fn named(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

fn is_yaml(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}

fn is_compose(name: &str) -> bool {
    (name.starts_with("docker-compose") && is_yaml(name))
        || name == "Dockerfile"
        || name.starts_with("Dockerfile.")
        || name == "compose.yml"
        || name == "compose.yaml"
}

fn read(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Sorted by variable (case-insensitive, then exact), then source; no duplicates.
/// Directories that cannot be read are named in `warnings`; a file that cannot be read is an error.
pub fn references(root: &Path, warnings: &mut Vec<String>) -> Result<Vec<Ref>> {
    let mut files = Vec::new();
    walk(root, 1, 4, &mut files, warnings)?;
    let mut jobs: Vec<(PathBuf, Parser)> = Vec::new();
    for (path, depth) in files {
        let name = named(&path);
        if ENV_TEMPLATES.contains(&name) {
            jobs.push((path, env_names));
        } else if depth <= 3 && is_compose(name) {
            jobs.push((path, compose_names));
        }
    }
    let workflows = root.join(".github/workflows");
    if std::fs::symlink_metadata(&workflows).is_ok_and(|m| m.is_dir()) {
        let mut found = Vec::new();
        walk(&workflows, 1, usize::MAX, &mut found, warnings)?;
        jobs.extend(
            found
                .into_iter()
                .filter(|(p, _)| is_yaml(named(p)))
                .map(|(p, _)| (p, workflow_names as Parser)),
        );
    }
    let mut refs = Vec::new();
    for (path, parse) in jobs {
        let source = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        refs.extend(parse(&read(&path)?).into_iter().map(|var| Ref {
            var,
            source: source.clone(),
        }));
    }
    refs.sort_by(|a, b| {
        (a.var.to_lowercase(), &a.var, &a.source).cmp(&(b.var.to_lowercase(), &b.var, &b.source))
    });
    refs.dedup();
    Ok(refs)
}

#[cfg(test)]
mod tests {
    include!("refs_tests.rs");
}
