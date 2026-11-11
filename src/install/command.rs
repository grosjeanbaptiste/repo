//! `repo install`: the registry, real `git`, and the real heavy queue with cargo's output on the terminal.
use super::{Opts, run};
use crate::registry::file::{path, read};
use crate::scaffold::native::Quiet;
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

/// Runs with the terminal's stdin, stdout and stderr: a long build must show its progress.
struct Live;

impl Exec for Live {
    fn run(&self, prog: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
        let mut cmd = Command::new(prog);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let status = cmd
            .status()
            .with_context(|| format!("{prog} not found: repo needs it"))?;
        if !status.success() {
            bail!("{prog} {} exited with {status}", args.join(" "));
        }
        Ok(String::new())
    }
}

pub fn command(opts: &Opts) -> Result<()> {
    let entries = read(&path(|k| std::env::var(k).ok())?)?;
    run(opts, &entries, &Quiet, &Live, &mut std::io::stdout().lock())
}
