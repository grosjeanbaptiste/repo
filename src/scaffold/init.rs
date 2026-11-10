//! Writes a plan into a target directory (`project-scaffold.sh init`):
//! overwrites, renders `.tmpl`, keeps symlinks as symlinks, keeps the exec bit.
use super::plan::Entry;
use super::render::{Vars, render};
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

/// A template symlink points at a sibling `.tmpl`; the deployed link points at
/// the sibling *rendered* file (`CLAUDE.md.tmpl -> AGENTS.md.tmpl` becomes
/// `CLAUDE.md -> AGENTS.md`).
pub fn link_target(entry: &Entry) -> Result<String> {
    let to =
        fs::read_link(&entry.src).with_context(|| format!("readlink {}", entry.src.display()))?;
    let to = to.to_string_lossy();
    Ok(to.strip_suffix(".tmpl").unwrap_or(&to).to_string())
}

pub fn is_link(entry: &Entry) -> Result<bool> {
    Ok(fs::symlink_metadata(&entry.src)
        .with_context(|| format!("stat {}", entry.src.display()))?
        .file_type()
        .is_symlink())
}

/// What the file should contain once rendered.
pub fn content(entry: &Entry, vars: &Vars) -> Result<Vec<u8>> {
    if entry.template {
        let text = fs::read_to_string(&entry.src)
            .with_context(|| format!("reading template {}", entry.src.display()))?;
        Ok(render(&text, vars).into_bytes())
    } else {
        fs::read(&entry.src).with_context(|| format!("reading {}", entry.src.display()))
    }
}

fn link(entry: &Entry, dst: &Path) -> Result<()> {
    let to = link_target(entry)?;
    if fs::symlink_metadata(dst).is_ok() {
        fs::remove_file(dst).with_context(|| format!("replacing {}", dst.display()))?;
    }
    symlink(&to, dst).with_context(|| format!("linking {}", dst.display()))
}

pub fn write(entry: &Entry, dst: &Path, vars: &Vars) -> Result<()> {
    fs::write(dst, content(entry, vars)?).with_context(|| format!("writing {}", dst.display()))?;
    let mode = fs::metadata(&entry.src)
        .with_context(|| format!("stat {}", entry.src.display()))?
        .permissions()
        .mode();
    if mode & 0o111 != 0 {
        fs::set_permissions(dst, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("chmod {}", dst.display()))?;
    }
    Ok(())
}

/// `{{project_name}}` in a destination path names the file after the project.
pub fn destination(target: &Path, entry: &Entry, vars: &Vars) -> std::path::PathBuf {
    let rel = entry.dst.to_string_lossy();
    let name = vars.get("project_name").map_or("", String::as_str);
    target.join(rel.replace("{{project_name}}", name))
}

pub fn init(target: &Path, plan: &[Entry], vars: &Vars) -> Result<()> {
    for entry in plan {
        let dst = destination(target, entry, vars);
        if let Some(dir) = dst.parent() {
            fs::create_dir_all(dir).with_context(|| format!("mkdir {}", dir.display()))?;
        }
        if is_link(entry)? {
            link(entry, &dst)?;
        } else {
            write(entry, &dst, vars)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("init_tests.rs");
}
