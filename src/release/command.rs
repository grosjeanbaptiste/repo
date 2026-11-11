//! `repo release`: the real audit, the machine profile of the state root, and `exec`.
use super::args::{Opts, Parsed, parse};
use super::launch::Launch;
use super::{Ports, run};
use crate::audit;
use crate::registry::file::state_root;
use anyhow::{Context, Result, bail};
use std::os::unix::process::CommandExt;
use std::process::Command;

const USAGE: &str = "Usage:
  repo release [--require-audit] [--path DIR] [--script PATH] [args...]

Runs the project's release script with the machine profile
($DOTFILES_STATE_ROOT/shell/profile.sh) and the project's .envrc loaded into
the environment, replacing the manual `set -a; source ...; source .envrc`.
The script is discovered by convention in DIR; pass --script to override.
Anything that is not one of these flags goes to the release script verbatim
(use `--` if its arguments collide with ours).

Conventions (looked up in order, in DIR):
  stores/scripts/tf_release.py    iOS TestFlight (mobile template)

Options:
  --require-audit   Run `repo audit` first; abort if a variable is missing or a check FAILs.
  --path DIR        Use DIR instead of the current directory.
  --script PATH     Override the script location.
";

struct Exec;

impl Launch for Exec {
    fn launch(&self, argv: &[String]) -> Result<()> {
        let Some((prog, rest)) = argv.split_first() else {
            bail!("release: nothing to launch");
        };
        let failure = Command::new(prog).args(rest).exec(); // returns only when it could not start
        Err(failure).with_context(|| format!("release: cannot start {prog}"))
    }
}

pub fn command(args: &[String]) -> Result<()> {
    let opts: Opts = match parse(args)? {
        Parsed::Help => {
            print!("{USAGE}");
            return Ok(());
        }
        Parsed::Run(o) => o,
    };
    let profile = state_root(|k| std::env::var(k).ok())?.join("shell/profile.sh");
    let audit =
        |dir: &std::path::Path, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write| {
            audit::command::run_to(dir, out, err)
        };
    let ports = Ports {
        profile,
        audit: &audit,
        launch: &Exec,
    };
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    run(&opts, &ports, &mut out, &mut err)
}
