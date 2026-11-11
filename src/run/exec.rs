//! Runs one verb of a project (port of `project_tools_run_exec`).
use super::targets::{self, Hook, Kind};
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::Path;

/// Runs a program with inherited stdio and returns its exit code; `Err` only when it cannot start.
pub trait Runner {
    fn run(&self, prog: &str, args: &[String], cwd: Option<&Path>) -> Result<i32>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// The verb is not a command of this project; nothing was started.
    Unknown,
    /// Started and failed (or refused with a usage error): the exit code to propagate.
    Failed(i32),
}

fn delegate(dir: &Path, verb: &str, args: &[String], runner: &dyn Runner) -> Result<Outcome> {
    let mut all = vec![
        "project".to_string(),
        dir.display().to_string(),
        verb.to_string(),
    ];
    all.extend(args.iter().cloned());
    finish(runner.run("dotfiles", &all, None)?)
}

fn finish(code: i32) -> Result<Outcome> {
    Ok(if code == 0 {
        Outcome::Done
    } else {
        Outcome::Failed(code)
    })
}

fn usage(err: &mut dyn Write, text: String) -> Result<Outcome> {
    writeln!(err, "{text}").context("writing the usage")?;
    Ok(Outcome::Failed(2))
}

/// `install` and `publish` keep their universal contract, whatever the kind.
fn universal(
    dir: &Path,
    verb: &str,
    args: &[String],
    runner: &dyn Runner,
    err: &mut dyn Write,
) -> Result<Option<Outcome>> {
    let shown = dir.display();
    if verb == "publish" {
        if args.len() < 2 {
            return usage(
                err,
                format!("Usage: repo run {shown} publish <prod|test> <store> [options]"),
            )
            .map(Some);
        }
        return delegate(dir, verb, args, runner).map(Some);
    }
    let hook = targets::hook(dir);
    if verb != "install" || (hook == Hook::Absent && !dir.join("pubspec.yaml").is_file()) {
        return Ok(None);
    }
    if let Hook::NotExecutable(p) = hook {
        bail!("device hook {} is not executable", p.display());
    }
    if args.len() != 1 {
        return usage(err, format!("Usage: repo run {shown} install <device>")).map(Some);
    }
    delegate(dir, verb, args, runner).map(Some)
}

fn in_dir(
    dir: &Path,
    prog: &str,
    args: &[&str],
    rest: &[String],
    runner: &dyn Runner,
) -> Result<Outcome> {
    let all: Vec<String> = args
        .iter()
        .map(|a| a.to_string())
        .chain(rest.iter().cloned())
        .collect();
    finish(runner.run(prog, &all, Some(dir))?)
}

fn ignored(err: &mut dyn Write, prog: &str, args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }
    writeln!(
        err,
        "warn: {prog} takes no extra arguments here, ignored: {}",
        args.join(" ")
    )
    .context("writing the warning")
}

fn kind_verb(
    dir: &Path,
    kind: Kind,
    verb: &str,
    args: &[String],
    runner: &dyn Runner,
    err: &mut dyn Write,
) -> Result<Option<Outcome>> {
    Ok(Some(match (kind, verb) {
        (Kind::Flutter, "run") => delegate(dir, verb, args, runner)?,
        (Kind::Flutter, "test" | "build" | "clean" | "pub-get") => {
            ignored(err, "flutter", args)?;
            let words: &[&str] = if verb == "pub-get" {
                &["pub", "get"]
            } else {
                &[verb]
            };
            in_dir(dir, "flutter", words, &[], runner)?
        }
        (Kind::Cargo, "run" | "build" | "test" | "check" | "clippy") => {
            in_dir(dir, "cargo", &[verb], args, runner)?
        }
        _ => return Ok(None),
    }))
}

fn makefile_target(
    dir: &Path,
    verb: &str,
    args: &[String],
    runner: &dyn Runner,
) -> Result<Outcome> {
    let mut all = vec![
        "-C".to_string(),
        dir.display().to_string(),
        verb.to_string(),
    ];
    if !args.is_empty() {
        all.push(format!("ARGS={}", args.join(" ")));
    }
    finish(runner.run("make", &all, None)?)
}

/// Runs `verb` with `args` in `dir`; usage errors and ignored arguments are said on `err`.
pub fn dispatch(
    dir: &Path,
    verb: &str,
    args: &[String],
    runner: &dyn Runner,
    err: &mut dyn Write,
) -> Result<Outcome> {
    if let Some(done) = universal(dir, verb, args, runner, err)? {
        return Ok(done);
    }
    let kind = targets::detect(dir);
    if kind == Kind::Makefile {
        let mut warnings = vec![];
        let known = targets::makefile_targets(&dir.join("Makefile"), &mut warnings)?;
        if known.iter().any(|t| t == verb) {
            return makefile_target(dir, verb, args, runner);
        }
    } else if let Some(done) = kind_verb(dir, kind, verb, args, runner, err)? {
        return Ok(done);
    }
    let melos = dir.join("melos.yaml");
    if melos.is_file() && targets::melos_scripts(&melos)?.iter().any(|s| s == verb) {
        ignored(err, "melos", args)?;
        return in_dir(dir, "melos", &["run", verb], &[], runner);
    }
    Ok(Outcome::Unknown)
}

#[cfg(test)]
mod tests {
    include!("exec_tests.rs");
}
