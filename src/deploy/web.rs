//! `deploy ... web`: Cloudflare Pages through wrangler (port of `project-deploy-web.sh`).
use super::target::Env;
use super::tool::{Call, Tool, plain};
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Static output folders, first existing wins: Flutter, Vite, CRA, Next export, plain static.
pub const CANDIDATES: [&str; 5] = ["build/web", "dist", "build", "out", "public"];

pub fn resolve_dir(root: &Path) -> Result<PathBuf> {
    for candidate in CANDIDATES {
        let dir = root.join(candidate);
        if dir.is_dir() {
            return dir
                .canonicalize()
                .with_context(|| format!("resolving {}", dir.display()));
        }
    }
    bail!(
        "No web build output under {} (looked for: {}). Build first.",
        root.display(),
        CANDIDATES.join(" ")
    )
}

/// prod -> `main`; test -> the project's current git branch, else `preview`.
pub fn branch(env: Env, root: &Path, tool: &dyn Tool) -> Result<String> {
    if env == Env::Prod {
        return Ok("main".into());
    }
    let args: Vec<String> = ["rev-parse", "--abbrev-ref", "HEAD"]
        .map(String::from)
        .to_vec();
    let call = Call {
        cwd: Some(root),
        ..plain("git", &args)
    };
    // Not a repository, or no commit yet: git exits non-zero and the preview branch is the answer.
    let current = tool.capture(&call)?.unwrap_or_default();
    Ok(match current.as_str() {
        "" | "HEAD" => "preview".into(),
        name => name.into(),
    })
}

pub fn wrangler_args(dir: &Path, name: &str, branch: &str) -> Vec<String> {
    vec![
        "pages".into(),
        "deploy".into(),
        dir.display().to_string(),
        format!("--project-name={name}"),
        format!("--branch={branch}"),
    ]
}

/// Builds nothing: publishes the existing build output. `dry_run` prints the command instead.
pub fn deploy(
    env: Env,
    root: &Path,
    name: Option<&str>,
    dry_run: bool,
    tool: &dyn Tool,
    out: &mut dyn Write,
) -> Result<()> {
    if !tool.on_path("wrangler") {
        bail!("wrangler not found\nInstall it with: dotfiles add --manager npm wrangler");
    }
    let dir = resolve_dir(root)?;
    let name = match name {
        Some(n) => n.to_string(),
        None => root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let args = wrangler_args(&dir, &name, &branch(env, root, tool)?);
    if dry_run {
        return writeln!(out, "wrangler {}", args.join(" ")).context("writing the plan");
    }
    tool.run(&plain("wrangler", &args))
}

#[cfg(test)]
mod tests {
    include!("web_tests.rs");
}
