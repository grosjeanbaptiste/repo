//! What a project can run: its kind and the verbs that really exist for it (port of `project-run-tools.sh`).
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Makefile,
    Flutter,
    Cargo,
    Unknown,
}

/// Makefile first, then Flutter, then Cargo.
pub fn detect(dir: &Path) -> Kind {
    if dir.join("Makefile").is_file() {
        Kind::Makefile
    } else if dir.join("pubspec.yaml").is_file() {
        Kind::Flutter
    } else if dir.join("Cargo.toml").is_file() {
        Kind::Cargo
    } else {
        Kind::Unknown
    }
}

/// A rule line `name:` / `name: deps` / `name::`, never an assignment (`:=`), a pattern or a special target.
fn rule_name(line: &str) -> Option<&str> {
    let (name, rest) = line.split_once(':')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    let word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-');
    let named = (first.is_ascii_alphabetic() || first == '_') && name.chars().all(word);
    (named && !rest.starts_with('=')).then_some(name)
}

/// `include FILE` / `-include FILE` with exactly one literal file name: `(file, optional)`.
fn include_of(line: &str) -> Option<(&str, bool)> {
    let (optional, rest) = match line.strip_prefix("-include") {
        Some(r) => (true, r),
        None => (false, line.strip_prefix("include")?),
    };
    let rest = rest.strip_prefix([' ', '\t'])?.trim();
    let literal = !rest.is_empty() && !rest.contains(char::is_whitespace);
    (literal && !rest.contains(['$', '*', '?', '['])).then_some((rest, optional))
}

struct Walk<'a> {
    seen: HashSet<String>,
    stack: Vec<PathBuf>,
    names: Vec<String>,
    warnings: &'a mut Vec<String>,
}

impl Walk<'_> {
    fn file(&mut self, path: &Path) -> Result<()> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        self.stack.push(path.to_path_buf());
        // Own targets first, then the included files (same order as the bash tool).
        let mut includes = vec![];
        for line in text.lines() {
            if let Some(name) = rule_name(line) {
                if self.seen.insert(name.to_string()) {
                    self.names.push(name.to_string());
                }
            } else if let Some(inc) = include_of(line) {
                includes.push(inc);
            }
        }
        for (file, optional) in includes {
            self.include(path, file, optional)?;
        }
        self.stack.pop();
        Ok(())
    }

    fn include(&mut self, from: &Path, file: &str, optional: bool) -> Result<()> {
        let target = from.parent().unwrap_or(Path::new(".")).join(file);
        if self.stack.contains(&target) {
            self.warnings
                .push(format!("include cycle at {}", target.display()));
        } else if target.is_file() {
            self.file(&target)?;
        } else if !optional {
            self.warnings.push(format!(
                "include {} not found (from {})",
                target.display(),
                from.display()
            ));
        }
        Ok(())
    }
}

/// Real target names of a Makefile and of the files it `include`s, in order, without duplicates.
/// A missing `-include` is normal; a missing plain `include` or a cycle is said in `warnings`.
pub fn makefile_targets(makefile: &Path, warnings: &mut Vec<String>) -> Result<Vec<String>> {
    let mut walk = Walk {
        seen: HashSet::new(),
        stack: vec![],
        names: vec![],
        warnings,
    };
    walk.file(makefile)?;
    Ok(walk.names)
}

/// Names under the top-level `scripts:` of a `melos.yaml`.
pub fn melos_scripts(melos_yaml: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(melos_yaml)
        .with_context(|| format!("cannot read {}", melos_yaml.display()))?;
    let mut inside = false;
    let mut names = vec![];
    for line in text.lines() {
        if !line.starts_with([' ', '\t']) && !line.is_empty() {
            inside = line.trim_end() == "scripts:";
        } else if inside && let Some(rest) = line.strip_prefix("  ") {
            let key = rest.trim_end().strip_suffix(':').filter(|k| {
                !k.is_empty()
                    && k.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
            });
            names.extend(key.map(str::to_string));
        }
    }
    Ok(names)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Hook {
    Absent,
    Ready,
    NotExecutable(PathBuf),
}

/// The project's device-install hook, `scripts/device-install.sh`.
pub fn hook(dir: &Path) -> Hook {
    let path = dir.join("scripts/device-install.sh");
    match fs::metadata(&path) {
        Err(_) => Hook::Absent, // absent (or unreachable) hook: the verb is just not offered
        Ok(m) if m.permissions().mode() & 0o111 != 0 => Hook::Ready,
        Ok(_) => Hook::NotExecutable(path),
    }
}

fn device_hook(dir: &Path, warnings: &mut Vec<String>) -> bool {
    match hook(dir) {
        Hook::Ready => true,
        Hook::Absent => false,
        Hook::NotExecutable(p) => {
            warnings.push(format!("device hook {} is not executable", p.display()));
            false
        }
    }
}

/// Every verb available in `dir`, without duplicates: kind verbs, melos scripts, `install`/`publish`.
pub fn list(dir: &Path, warnings: &mut Vec<String>) -> Result<Vec<String>> {
    let fixed = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut all = match detect(dir) {
        Kind::Makefile => makefile_targets(&dir.join("Makefile"), warnings)?,
        Kind::Flutter => fixed(&["run", "test", "build", "clean", "pub-get"]),
        Kind::Cargo => fixed(&["run", "build", "test", "check", "clippy"]),
        Kind::Unknown => vec![],
    };
    if dir.join("melos.yaml").is_file() {
        all.extend(melos_scripts(&dir.join("melos.yaml"))?);
    }
    if dir.join("pubspec.yaml").is_file() {
        all.extend(fixed(&["install", "publish"]));
    }
    if fs::File::open(dir.join("stores/scripts/tf_release.py")).is_ok() {
        all.push("publish".into());
    }
    if device_hook(dir, warnings) {
        all.push("install".into());
    }
    let mut seen = HashSet::new();
    all.retain(|v| seen.insert(v.clone()));
    Ok(all)
}

#[cfg(test)]
mod tests {
    include!("targets_tests.rs");
}
