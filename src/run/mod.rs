//! `repo run NAME-OR-DIR [VERB [ARGS...]]` (port of dotfiles `project <folder> <verb>`).
pub mod command;
pub mod exec;
#[cfg(test)]
mod fake;
pub mod targets;

use crate::registry::file::{Entry, find};
use anyhow::{Context, Result, bail};
use exec::{Outcome, Runner, dispatch};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A directory first (`.` included), else a registered name.
fn resolve(name: &str, entries: &[Entry]) -> Result<PathBuf> {
    if Path::new(name).is_dir() {
        return std::fs::canonicalize(name).with_context(|| format!("resolving {name}"));
    }
    let Some(path) = find(entries, name) else {
        bail!(
            "Unknown project: {name}\nRun `repo list` to see registered projects, or `repo scan` to refresh it."
        );
    };
    if !path.is_dir() {
        bail!(
            "Registered path for '{name}' no longer exists: {}\nRun `repo scan` to refresh the registry.",
            path.display()
        );
    }
    Ok(path)
}

fn verbs(dir: &Path, err: &mut dyn Write) -> Result<Vec<String>> {
    let mut warnings = vec![];
    let all = targets::list(dir, &mut warnings)?;
    for w in warnings {
        writeln!(err, "warn: {w}").context("writing a warning")?;
    }
    Ok(all)
}

/// Without a verb: lists the project's verbs. With one: runs it; an unknown verb is reported with the available ones.
pub fn run(
    name: &str,
    verb: Option<&str>,
    args: &[String],
    entries: &[Entry],
    runner: &dyn Runner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Outcome> {
    let dir = resolve(name, entries)?;
    let Some(verb) = verb else {
        let all = verbs(&dir, err)?;
        if all.is_empty() {
            bail!("No commands found for {name} ({})", dir.display());
        }
        writeln!(out, "{}", all.join("\n")).context("writing the verbs")?;
        return Ok(Outcome::Done);
    };
    let outcome = dispatch(&dir, verb, args, runner, err)?;
    if outcome == Outcome::Unknown {
        let all = verbs(&dir, err)?;
        writeln!(err, "'{verb}' isn't a known command for {name}.")
            .context("writing the report")?;
        if !all.is_empty() {
            writeln!(err, "Available:\n{}", all.join("\n")).context("writing the report")?;
        }
        return Ok(Outcome::Failed(1));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
