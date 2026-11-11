//! Which release script to run, and how.
use anyhow::{Result, bail};
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Conventional locations in the project, in order.
pub const CONVENTIONS: [&str; 1] = ["stores/scripts/tf_release.py"];

/// `over` (absolute, or relative to `target`) when given, else the first convention present.
pub fn resolve(target: &Path, over: Option<&str>) -> Result<PathBuf> {
    let readable = |p: &PathBuf| File::open(p).is_ok();
    // `join` keeps an absolute override as it is.
    let found = match over {
        Some(o) => Some(target.join(o)).filter(readable),
        None => CONVENTIONS.iter().map(|c| target.join(c)).find(readable),
    };
    match found {
        Some(p) => Ok(p),
        None => bail!(
            "no release script found in {}\n  Tried: {}\n  Override with --script PATH if yours lives elsewhere.",
            target.display(),
            CONVENTIONS.join(", ")
        ),
    }
}

/// Program and arguments that run `script`: python3 for `.py`, bash for `.sh`/`.bash`, else itself when executable.
pub fn command(script: &Path) -> Result<Vec<String>> {
    let path = script.to_string_lossy().into_owned();
    match script.extension().and_then(|e| e.to_str()) {
        Some("py") => return Ok(vec!["python3".into(), path]),
        Some("sh" | "bash") => return Ok(vec!["bash".into(), path]),
        _ => {}
    }
    let executable = script
        .metadata()
        .is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
    if !executable {
        bail!("cannot determine how to run {path} (not .py/.sh, not executable)");
    }
    Ok(vec![path])
}

#[cfg(test)]
mod tests {
    include!("script_tests.rs");
}
