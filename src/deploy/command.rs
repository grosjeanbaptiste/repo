//! `repo deploy`: the real processes, environment and terminal.
use super::target::{Choice, Env};
use super::tool::{Call, Tool};
use super::vars::Vars;
use super::{Opts, run};
use anyhow::{Context, Result, bail};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};

struct Process;

fn command(call: &Call) -> Command {
    let mut cmd = Command::new(call.prog);
    cmd.args(call.args)
        .envs(call.env.iter().map(|(k, v)| (k, v)));
    if let Some(dir) = call.cwd {
        cmd.current_dir(dir);
    }
    cmd
}

fn describe(prog: &str, status: ExitStatus) -> String {
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("{prog} exited with status {code}"),
        (None, Some(sig)) => format!("{prog} was killed by signal {sig}"),
        (None, None) => format!("{prog} ended without an exit status"),
    }
}

impl Tool for Process {
    fn run(&self, call: &Call) -> Result<()> {
        let status = command(call)
            .status()
            .with_context(|| format!("cannot start {}", call.prog))?;
        if !status.success() {
            bail!("{}", describe(call.prog, status));
        }
        Ok(())
    }

    fn capture(&self, call: &Call) -> Result<Option<String>> {
        let out = command(call)
            .stderr(Stdio::null()) // a non-zero exit is the answer; the caller says what it means
            .output()
            .with_context(|| format!("cannot start {}", call.prog))?;
        Ok(out
            .status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }

    fn on_path(&self, prog: &str) -> bool {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path).any(|dir| {
            std::fs::metadata(dir.join(prog))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    }
}

pub struct Args {
    pub env: String,
    pub target: String,
    pub path: String,
    pub project: Option<String>,
    pub yes: bool,
    pub dry_run: bool,
    pub store_page: bool,
}

pub fn command_line(args: Args) -> Result<()> {
    let opts = Opts {
        env: Env::parse(&args.env)?,
        choice: Choice::parse(&args.target)?,
        path: args.path.into(),
        project: args.project,
        yes: args.yes,
        dry_run: args.dry_run,
        store_page: args.store_page,
    };
    let vars = Vars::from_process()?;
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    run(
        &opts,
        &vars,
        &Process,
        &mut std::io::stdin().lock(),
        &mut out,
        &mut err,
    )
}
