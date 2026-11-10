//! Which template files land where: type layer, then `common/`, then the
//! `skeleton/` of the type. First layer to ship a relative path wins.
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const LEAF_KINDS: [&str; 1] = ["workspace"];

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub src: PathBuf,
    /// Destination relative to the target, `.tmpl` stripped.
    pub dst: PathBuf,
    pub template: bool,
}

fn subdirs(dir: &Path) -> Result<Vec<String>> {
    let Ok(read) = fs::read_dir(dir) else {
        return Ok(Vec::new()); // a missing layer is simply empty
    };
    let mut names = Vec::new();
    for e in read {
        let e = e.with_context(|| format!("reading {}", dir.display()))?;
        if e.path().is_dir() {
            names.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// `app/python`, `doc/tex`, …, and the bare leaf kinds (`workspace`).
pub fn known_types(root: &Path) -> Result<Vec<String>> {
    let mut types = Vec::new();
    for kind in subdirs(root)?.into_iter().filter(|k| k != "common") {
        if LEAF_KINDS.contains(&kind.as_str()) {
            types.push(kind);
            continue;
        }
        for lang in subdirs(&root.join(&kind))? {
            types.push(format!("{kind}/{lang}"));
        }
    }
    Ok(types)
}

fn files_under(base: &Path, skip: Option<&Path>, out: &mut Vec<(PathBuf, PathBuf)>) -> Result<()> {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .collect::<std::io::Result<_>>()
            .with_context(|| format!("reading {}", dir.display()))?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let kind = e
                .file_type()
                .with_context(|| format!("stat {}", path.display()))?;
            if kind.is_dir() {
                if skip != Some(path.as_path()) {
                    stack.push(path);
                }
            } else {
                let rel = path.strip_prefix(base).map(Path::to_path_buf);
                out.push((rel.with_context(|| "path outside its layer")?, path));
            }
        }
    }
    Ok(())
}

pub fn plan(root: &Path, kind: &str) -> Result<Vec<Entry>> {
    if !known_types(root)?.iter().any(|t| t == kind) {
        bail!("Unknown project type: {kind}");
    }
    layered(root, kind, true)
}

/// No type validation, optional skeleton: `check`/`apply` accept detected
/// names (`tex`, `python`, …) that may ship only the `common/` layer.
pub fn layered(root: &Path, kind: &str, skeleton_too: bool) -> Result<Vec<Entry>> {
    let type_root = root.join(kind);
    let skeleton = type_root.join("skeleton");
    let mut files = Vec::new();
    if type_root.is_dir() {
        files_under(&type_root, Some(&skeleton), &mut files)?;
    }
    if root.join("common").is_dir() {
        files_under(&root.join("common"), None, &mut files)?;
    }
    if skeleton_too && skeleton.is_dir() {
        files_under(&skeleton, None, &mut files)?;
    }
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for (rel, src) in files {
        if !seen.insert(rel.clone()) {
            continue;
        }
        let template = rel.extension().is_some_and(|e| e == "tmpl");
        let dst = if template {
            rel.with_extension("")
        } else {
            rel
        };
        entries.push(Entry { src, dst, template });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    include!("plan_tests.rs");
}
