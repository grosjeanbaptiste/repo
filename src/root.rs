//! `--root` handling: `~` expansion without surprises.
use anyhow::{Result, bail};
use std::path::PathBuf;

/// `~` and `~/…` use `home`; `~user` is refused; anything else is kept as is.
pub fn expand(root: &str, home: Option<String>) -> Result<PathBuf> {
    let Some(rest) = root.strip_prefix('~') else {
        return Ok(PathBuf::from(root));
    };
    if !rest.is_empty() && !rest.starts_with('/') {
        let user = rest.split('/').next().unwrap_or("");
        bail!("cannot expand ~{user} in --root: only ~ and ~/… are supported");
    }
    let Some(home) = home else {
        bail!("HOME is not set: cannot expand ~ in --root (pass an absolute --root)");
    };
    Ok(PathBuf::from(home).join(rest.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {
    include!("root_tests.rs");
}
