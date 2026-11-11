//! Facts shared by the store channels: the uploader scripts, their python, package id and version.
use super::tool::{Tool, plain};
use super::vars::Vars;
use anyhow::{Context, Result};
use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Where the dotfiles stores scripts live (`STORES_SCRIPTS_DIR` overrides).
pub fn scripts_dir(v: &Vars) -> PathBuf {
    if let Some(dir) = v.var("STORES_SCRIPTS_DIR") {
        return PathBuf::from(dir);
    }
    let root = v.path_or("DOTFILES_REPO_ROOT", "dotfiles");
    root.join("project-templates/mobile/native/stores/scripts")
}

fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// A python with the stores dependencies: `STORES_PYTHON`, else a managed venv provisioned once.
pub fn python(v: &Vars, tool: &dyn Tool, err: &mut dyn Write) -> Result<PathBuf> {
    if let Some(py) = v.var("STORES_PYTHON") {
        return Ok(PathBuf::from(py));
    }
    let venv = v.home.join(".cache/dotfiles/stores-venv");
    let py = venv.join("bin/python");
    if executable(&py) {
        return Ok(py);
    }
    writeln!(
        err,
        "deploy: provisioning stores venv at {} …",
        venv.display()
    )
    .context("writing the notice")?;
    let make = [
        "-m".to_string(),
        "venv".to_string(),
        venv.display().to_string(),
    ];
    tool.run(&plain("python3", &make))
        .context("creating the stores venv")?;
    let requirements = scripts_dir(v)
        .join("requirements.txt")
        .display()
        .to_string();
    let pip = ["-m", "pip", "install", "-q", "-r", requirements.as_str()].map(String::from);
    let prog = py.display().to_string();
    tool.run(&plain(&prog, &pip))
        .context("installing the stores requirements")?;
    Ok(py)
}

/// The value of `applicationId "x"` / `applicationId = "x"` on a line.
fn application_id(line: &str) -> Option<&str> {
    let rest = line.split_once("applicationId")?.1;
    let after = rest.trim_start_matches([' ', '\t', '=']);
    let separated = after.len() < rest.len();
    let quoted = after.strip_prefix('"')?;
    let id = &quoted[..quoted.find('"')?];
    (separated && !id.is_empty()).then_some(id)
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// `PLAY_PACKAGE_NAME`, else the first `applicationId` of `android/app/build.gradle*`.
pub fn android_package(v: &Vars, root: &Path) -> Result<Option<String>> {
    if let Some(name) = v.var("PLAY_PACKAGE_NAME") {
        return Ok(Some(name));
    }
    let dir = root.join("android/app");
    let Some(entries) = read_optional_dir(&dir)? else {
        return Ok(None);
    };
    let mut files: Vec<PathBuf> = entries
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("build.gradle"))
        })
        .collect();
    files.sort();
    for file in files {
        let text = read_optional(&file)?.unwrap_or_default();
        if let Some(id) = text.lines().find_map(application_id) {
            return Ok(Some(id.to_string()));
        }
    }
    Ok(None)
}

fn read_optional_dir(dir: &Path) -> Result<Option<Vec<PathBuf>>> {
    match fs::read_dir(dir) {
        Ok(read) => read
            .map(|e| {
                e.map(|e| e.path())
                    .with_context(|| format!("reading {}", dir.display()))
            })
            .collect::<Result<_>>()
            .map(Some),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", dir.display())),
    }
}

/// The `version:` of `pubspec.yaml` without its `+build` part.
pub fn pubspec_version(root: &Path) -> Result<Option<String>> {
    let Some(text) = read_optional(&root.join("pubspec.yaml"))? else {
        return Ok(None);
    };
    let line = text.lines().find_map(|l| l.strip_prefix("version:"));
    let version = line.map(|l| {
        l.trim_start()
            .split('+')
            .next()
            .unwrap_or_default()
            .to_string()
    });
    Ok(version.filter(|v| !v.is_empty()))
}

#[cfg(test)]
mod tests {
    include!("stores_tests.rs");
}
