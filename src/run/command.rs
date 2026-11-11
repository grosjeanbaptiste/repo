//! `repo run`: the real registry, process and exit code.
use super::exec::{Outcome, Runner};
use crate::registry::file::{path, read};
use anyhow::{Context, Result, bail};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;

/// Starts the program with the terminal's stdio and waits for it.
struct Inherit;

impl Runner for Inherit {
    fn run(&self, prog: &str, args: &[String], cwd: Option<&Path>) -> Result<i32> {
        let mut cmd = Command::new(prog);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let status = cmd
            .status()
            .with_context(|| format!("cannot start {prog}"))?;
        match (status.code(), status.signal()) {
            (Some(code), _) => Ok(code),
            (None, Some(sig)) => bail!("{prog} was killed by signal {sig}"),
            (None, None) => bail!("{prog} ended without an exit status"),
        }
    }
}

/// Returns the exit code `repo` must end with.
pub fn command(name: &str, rest: &[String]) -> Result<i32> {
    let (verb, args) = match rest.split_first() {
        Some((verb, args)) => (Some(verb.as_str()), args),
        None => (None, rest),
    };
    let entries = read(&path(|k| std::env::var(k).ok())?)?;
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    let outcome = super::run(name, verb, args, &entries, &Inherit, &mut out, &mut err)?;
    Ok(match outcome {
        Outcome::Done => 0,
        Outcome::Failed(code) => code,
        Outcome::Unknown => 1, // run() already turns Unknown into Failed(1) with the report
    })
}
