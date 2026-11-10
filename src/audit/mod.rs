//! `repo audit [path]` (port of dotfiles `project audit`): env vars a project references.
pub mod command;
pub mod diag;
pub mod parse;
pub mod refs;
pub mod report;
pub mod resolve;
pub mod which;

use diag::Host;
use resolve::Status;
use std::io::Write;
use std::path::Path;

/// Prints the report of `root`; fails when a variable is missing or a check failed.
/// Directories that could not be read go to `err` as `warn:` lines.
pub fn run(
    root: &Path,
    status: &dyn Fn(&str) -> Status,
    host: &Host,
    out: &mut impl Write,
    err: &mut impl Write,
) -> anyhow::Result<()> {
    let mut warnings = Vec::new();
    let refs = refs::references(root, &mut warnings)?;
    for w in &warnings {
        writeln!(err, "warn: {w}")?;
    }
    let findings = diag::run_all(root, host)?;
    let tally = report::render(root, &refs, &findings, status, out)?;
    let mut problems = Vec::new();
    if tally.missing > 0 {
        problems.push(format!("{} variable(s) missing", tally.missing));
    }
    if tally.failed > 0 {
        problems.push(format!("{} check(s) failed", tally.failed));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("audit: {}", problems.join(", "))
    }
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
