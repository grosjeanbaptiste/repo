//! The `GitHub` section of `doctor`: open and stale PRs, CI on the default branch.
use crate::scaffold::setup::Exec;
use actautoma::forge::Forge;
use std::path::Path;

fn row(key: &str, value: &str) -> String {
    format!("    {key:<22} {value}")
}

/// The open-PR lines; a failing query shows `?` and warns, never a made-up count.
fn pulls(forge: &dyn Forge, cutoff: &str, warnings: &mut Vec<String>) -> Vec<String> {
    match forge.open_prs() {
        Err(e) => {
            warnings.push(format!("doctor: cannot list open PRs: {e}"));
            vec![row("Open PRs:", "?")]
        }
        Ok(prs) => {
            let mine = prs.iter().filter(|p| p.mine).count();
            let stale = prs
                .iter()
                .filter(|p| p.updated_at.as_str() < cutoff)
                .count();
            let mut out = vec![row(
                "Open PRs:",
                &format!("{} total, {mine} mine", prs.len()),
            )];
            if stale > 0 {
                out.push(row("Stale:", &format!("{stale} PR(s) inactive >30 days")));
            }
            out
        }
    }
}

fn ci(forge: &dyn Forge, warnings: &mut Vec<String>) -> String {
    let branch = forge
        .view_repo()
        .map(|r| r.default_branch)
        .unwrap_or_else(|e| {
            warnings.push(format!(
                "doctor: cannot read the default branch ({e}); assuming main"
            ));
            "main".into()
        });
    let label = format!("CI on {branch}:");
    match forge.last_run(&branch) {
        Err(e) => {
            warnings.push(format!("doctor: cannot read the CI of {branch}: {e}"));
            row(&label, "?")
        }
        Ok(None) => row(&label, "(no runs)"),
        Ok(Some(r)) => row(&label, &format!("{}: {} ({})", r.workflow, r.state, r.date)),
    }
}

/// Lines of the section. A query that fails shows `?` and adds a `warn:` text to `warnings`.
pub fn lines(
    forge: &dyn Forge,
    exec: &dyn Exec,
    root: &Path,
    cutoff: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    if let Err(e) = forge.available() {
        return vec![format!(
            "    (platform tool unavailable: {e} — `dotfiles add gh` to enable)"
        )];
    }
    // `git config --get` exits 1 when the key is unset: no origin is an answer.
    let origin = exec
        .run("git", &["config", "--get", "remote.origin.url"], Some(root))
        .unwrap_or_else(|e| {
            let why = format!("{e:#}");
            if !why.contains("exit status: 1") {
                warnings.push(format!("doctor: cannot read the origin remote: {why}"));
            }
            String::new()
        });
    let origin = origin.trim();
    if origin.is_empty() {
        return vec!["    (no origin remote configured)".into()];
    }
    if !origin.contains("github.com") {
        return vec![format!("    (remote is {origin} — not GitHub)")];
    }
    let mut out = pulls(forge, cutoff, warnings);
    out.push(ci(forge, warnings));
    out
}

#[cfg(test)]
mod tests {
    include!("github_tests.rs");
}
