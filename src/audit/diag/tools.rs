//! Checks of the toolchain a project declares: versions files, lockfiles, direnv.
use super::{Finding, Host, fail, ok, warn};
use anyhow::{Context, Result};
use std::path::Path;

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// `have` is `want` or one of its refinements (`20` matches `20.11.1`, not `200`).
fn matches(have: &str, want: &str) -> bool {
    have == want
        || have
            .strip_prefix(want)
            .is_some_and(|rest| rest.starts_with('.'))
}

pub fn tool_versions(root: &Path, h: &Host) -> Result<Option<Finding>> {
    let file = root.join(".tool-versions");
    if !file.is_file() {
        return Ok(None);
    }
    let count = read(&file)?
        .lines()
        .filter(|l| {
            l.trim_start()
                .starts_with(|c: char| c.is_ascii_alphabetic())
        })
        .count();
    Ok(Some(if (h.has_tool)("mise") {
        ok(format!(
            ".tool-versions present ({count} tools) — `mise install` to provision"
        ))
    } else if (h.has_tool)("asdf") {
        ok(format!(
            ".tool-versions present ({count} tools) — `asdf install` to provision"
        ))
    } else {
        warn(format!(
            ".tool-versions present ({count} tools) but neither mise nor asdf installed (suggest: `dotfiles add mise`)"
        ))
    }))
}

/// The version a file asks for: whitespace removed, one leading `v` dropped.
fn wanted(text: &str) -> String {
    let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    squeezed.strip_prefix('v').unwrap_or(&squeezed).to_string()
}

pub fn nvmrc(root: &Path, h: &Host) -> Result<Option<Finding>> {
    let file = root.join(".nvmrc");
    if !file.is_file() {
        return Ok(None);
    }
    let want = wanted(&read(&file)?);
    if want.is_empty() {
        return Ok(Some(warn(".nvmrc is empty")));
    }
    if !(h.has_tool)("node") {
        return Ok(Some(warn(format!(
            ".nvmrc wants node {want} but node not installed"
        ))));
    }
    let have = match h.run("node", &["--version"], root) {
        Ok(out) => wanted(&out),
        Err(e) => return Ok(Some(fail(format!("`node --version` failed: {e:#}")))),
    };
    Ok(Some(if matches(&have, &want) {
        ok(format!("node {have} matches .nvmrc"))
    } else {
        warn(format!(
            "node {have} in PATH but .nvmrc wants {want} (suggest: `nvm use`)"
        ))
    }))
}

pub fn python_version(root: &Path, h: &Host) -> Result<Option<Finding>> {
    let file = root.join(".python-version");
    if !file.is_file() {
        return Ok(None);
    }
    let want: String = read(&file)?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if want.is_empty() {
        return Ok(Some(warn(".python-version is empty")));
    }
    let Some(bin) = ["python3", "python"].into_iter().find(|b| (h.has_tool)(b)) else {
        return Ok(Some(warn(format!(
            ".python-version wants {want} but no python in PATH"
        ))));
    };
    let out = match h.run(bin, &["--version"], root) {
        Ok(out) => out,
        Err(e) => return Ok(Some(fail(format!("`{bin} --version` failed: {e:#}")))),
    };
    let have = out.trim().strip_prefix("Python ").unwrap_or(out.trim());
    Ok(Some(if matches(have, &want) {
        ok(format!("python {have} matches .python-version"))
    } else {
        warn(format!(
            "python {have} in PATH but .python-version wants {want} (suggest: `pyenv install` or `mise use`)"
        ))
    }))
}

pub fn mise_toml(root: &Path, h: &Host) -> Option<Finding> {
    if !root.join("mise.toml").is_file() && !root.join(".mise.toml").is_file() {
        return None;
    }
    Some(if (h.has_tool)("mise") {
        ok("mise.toml present — `mise install` to provision")
    } else {
        warn("mise.toml present but mise not installed (suggest: `dotfiles add mise`)")
    })
}

const LOCKFILES: [(&str, &str); 11] = [
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    ("bun.lockb", "bun"),
    ("bun.lock", "bun"),
    ("package-lock.json", "npm"),
    ("poetry.lock", "poetry"),
    ("uv.lock", "uv"),
    ("Pipfile.lock", "pipenv"),
    ("Gemfile.lock", "bundle"),
    ("Cargo.lock", "cargo"),
    ("go.sum", "go"),
];

pub fn package_managers(root: &Path, h: &Host) -> Vec<Finding> {
    LOCKFILES
        .iter()
        .filter(|(lock, _)| root.join(lock).is_file())
        .map(|(lock, tool)| {
            if (h.has_tool)(tool) {
                ok(format!("{tool} available (declared via {lock})"))
            } else {
                warn(format!(
                    "{tool} declared via {lock} but not installed (suggest: `dotfiles add {tool}`)"
                ))
            }
        })
        .collect()
}

pub fn direnv(root: &Path, h: &Host) -> Option<Finding> {
    if !root.join(".envrc").is_file() {
        return None;
    }
    if !(h.has_tool)("direnv") {
        return Some(warn(
            ".envrc present but direnv not installed (suggest: `dotfiles add direnv`)",
        ));
    }
    let out = match h.run("direnv", &["status"], root) {
        Ok(out) => out,
        Err(e) => {
            return Some(warn(format!(
                ".envrc present but direnv reports unknown state: {e:#}"
            )));
        }
    };
    Some(if out.contains("Found RC allowed true") {
        ok("direnv loaded for this repo")
    } else if out.contains("Found RC allowed false") || out.contains("Found RC allowed 0") {
        warn(".envrc present but not allowed (run `direnv allow` inside the repo)")
    } else {
        warn(".envrc present but direnv reports unknown state")
    })
}

#[cfg(test)]
mod tests {
    include!("tools_tests.rs");
}
