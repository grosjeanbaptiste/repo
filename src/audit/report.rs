//! The two tables of `audit`: referenced variables, then findings.
use super::diag::{Finding, Level};
use super::refs::Ref;
use super::resolve::Status;
use anyhow::Result;
use std::io::Write;
use std::path::Path;

/// What made the audit fail, when it did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub missing: usize,
    pub failed: usize,
}

/// Variables in order with the sources that reference each, joined by ", ".
fn grouped(refs: &[Ref]) -> Vec<(&str, String)> {
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for r in refs {
        match groups.last_mut() {
            Some((var, sources)) if *var == r.var => sources.push(&r.source),
            _ => groups.push((&r.var, vec![&r.source])),
        }
    }
    groups.into_iter().map(|(v, s)| (v, s.join(", "))).collect()
}

fn variables(refs: &[Ref], status: &dyn Fn(&str) -> Status, out: &mut impl Write) -> Result<usize> {
    if refs.is_empty() {
        writeln!(out, "  (no env vars / secrets referenced in this tree)")?;
        return Ok(0);
    }
    writeln!(
        out,
        "\n  {:<32} {:<14} SOURCES / HINT\n  {:<32} {:<14} --------------",
        "VAR", "STATUS", "---", "------"
    )?;
    let (mut env, mut dotfiles, mut missing) = (0, 0, 0);
    for (var, sources) in grouped(refs) {
        match status(var) {
            Status::InEnv => {
                writeln!(out, "  {var:<32} {:<14} {sources}", "in env")?;
                env += 1;
            }
            Status::InDotfiles(secret) => {
                writeln!(
                    out,
                    "  {var:<32} {:<14} {sources}  (enrolled as {secret})",
                    "in dotfiles"
                )?;
                dotfiles += 1;
            }
            Status::Missing => {
                writeln!(out, "  {var:<32} {:<14} {sources}", "missing")?;
                missing += 1;
            }
        }
    }
    writeln!(
        out,
        "\n  {env} in env, {dotfiles} in dotfiles (not exported), {missing} missing"
    )?;
    Ok(missing)
}

fn findings_table(findings: &[Finding], out: &mut impl Write) -> Result<usize> {
    if findings.is_empty() {
        return Ok(0);
    }
    writeln!(
        out,
        "\n  {:<6} FINDING\n  {:<6} -------",
        "STATUS", "------"
    )?;
    for f in findings {
        writeln!(out, "  {:<6} {}", f.level, f.msg)?;
    }
    let count = |l: Level| findings.iter().filter(|f| f.level == l).count();
    let failed = count(Level::Fail);
    writeln!(
        out,
        "\n  {} ok, {} warn, {failed} fail",
        count(Level::Ok),
        count(Level::Warn)
    )?;
    Ok(failed)
}

pub fn render(
    root: &Path,
    refs: &[Ref],
    findings: &[Finding],
    status: &dyn Fn(&str) -> Status,
    out: &mut impl Write,
) -> Result<Tally> {
    writeln!(out, "== auditing {} ==", root.display())?;
    let missing = variables(refs, status, out)?;
    let failed = findings_table(findings, out)?;
    Ok(Tally { missing, failed })
}

#[cfg(test)]
mod tests {
    include!("report_tests.rs");
}
