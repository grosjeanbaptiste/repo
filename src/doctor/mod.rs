//! `repo doctor [path]` (port of dotfiles `project doctor`): one-glance project health report.
pub mod command;
pub mod date;
pub mod git;
pub mod github;
pub mod quality;

use crate::scaffold::setup::Exec;
use actautoma::forge::Forge;
use anyhow::Result;
use std::io::Write;
use std::path::Path;

/// What the report asks of the outside world, so it can be scripted.
pub struct Ports<'a> {
    pub exec: &'a dyn Exec,
    pub forge: &'a dyn Forge,
    /// `YYYY-MM-DDTHH:MM:SSZ`: a PR not updated since is stale.
    pub cutoff: String,
    /// Runs the audit, writing its report and its warnings.
    pub audit: &'a dyn Fn(&mut dyn Write, &mut dyn Write) -> Result<()>,
}

/// Prints the report of `root`; fails when a section failed or the audit found a problem.
pub fn run(root: &Path, ports: &Ports, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    writeln!(out, "== project doctor: {} ==", root.display())?;
    let mut failed: Vec<&str> = vec![];
    let mut warnings = vec![];
    let mut section = |name: &'static str, body: Result<Vec<String>>| -> Result<()> {
        writeln!(out, "\n  == {name} ==")?;
        let lines = body.unwrap_or_else(|e| {
            failed.push(name.split_whitespace().next().unwrap_or(name));
            vec![format!("    (error: {e:#})")]
        });
        lines.iter().try_for_each(|l| writeln!(out, "{l}"))?;
        Ok(())
    };
    section("Git", git::lines(ports.exec, root))?;
    let github = github::lines(ports.forge, ports.exec, root, &ports.cutoff, &mut warnings);
    section("GitHub", Ok(github))?;
    section("Quality signals", quality::lines(ports.exec, root))?;
    section(
        "License",
        crate::license::doctor_line(root).map(|l| vec![l]),
    )?;
    for w in &warnings {
        writeln!(err, "warn: {w}")?;
    }
    let audit = audit_section(ports, out)?;
    let mut problems = vec![];
    if !failed.is_empty() {
        problems.push(format!(
            "{} section(s) failed ({})",
            failed.len(),
            failed.join(", ")
        ));
    }
    if let Err(e) = audit {
        problems.push(format!("audit failed ({e:#})"));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("doctor: {}", problems.join("; "))
    }
}

/// The audit section; the audit's own verdict is returned for the caller to weigh.
fn audit_section(ports: &Ports, out: &mut dyn Write) -> Result<Result<()>> {
    writeln!(out, "\n  == Audit ==")?;
    let (mut report, mut warned) = (Vec::new(), Vec::new());
    let verdict = (ports.audit)(&mut report, &mut warned);
    for text in [report, warned] {
        for line in String::from_utf8_lossy(&text).lines() {
            writeln!(out, "    {line}")?;
        }
    }
    writeln!(out)?;
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
