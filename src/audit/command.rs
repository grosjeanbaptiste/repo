//! The real ports of `repo audit`: process environment, `PATH`, the secret store, stdout.
use super::diag::Host;
use super::{resolve, which};
use crate::registry::file::state_root;
use crate::scaffold::native::Quiet;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("audit: path is not a directory: {}", path.display());
    }
    let root = path
        .canonicalize()
        .with_context(|| format!("resolving {}", path.display()))?;
    let state = state_root(|k| std::env::var(k).ok())?;
    let status = |var: &str| {
        let in_env = |name: &str| std::env::var_os(name).is_some();
        resolve::status(var, &in_env, &|secret| resolve::enrolled(&state, secret))
    };
    let path_var = std::env::var_os("PATH");
    let has_tool = |tool: &str| which::on_path(tool, path_var.as_deref());
    let host = Host {
        has_tool: &has_tool,
        exec: &Quiet,
    };
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    super::run(&root, &status, &host, &mut out, &mut err)
}
