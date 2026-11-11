//! `repo deploy ENV TARGET [PATH]` (port of dotfiles `project deploy`).
pub mod asc;
pub mod brew;
pub mod command;
pub mod ctx;
#[cfg(test)]
mod fake;
pub mod find;
pub mod msstore;
pub mod native;
pub mod play;
pub mod snap;
pub mod store_page;
pub mod stores;
pub mod target;
pub mod tool;
pub mod vars;
pub mod web;

use anyhow::{Context, Result, bail};
use ctx::Ctx;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use target::{Choice, Env, Target};
use tool::Tool;
use vars::Vars;

pub struct Opts {
    pub env: Env,
    pub choice: Choice,
    pub path: PathBuf,
    /// Cloudflare Pages project name (web); the folder name when absent.
    pub project: Option<String>,
    pub yes: bool,
    pub dry_run: bool,
    pub store_page: bool,
}

/// The shell's `[y/N]`: only y, Y, yes and YES say yes; end of input is a no.
fn confirm(question: &str, input: &mut dyn BufRead, out: &mut dyn Write) -> Result<bool> {
    write!(out, "{question} [y/N] ")
        .and_then(|()| out.flush())
        .context("writing the question")?;
    let mut reply = String::new();
    input.read_line(&mut reply).context("reading the answer")?;
    Ok(matches!(
        reply.trim_end_matches(['\n', '\r']),
        "y" | "Y" | "yes" | "YES"
    ))
}

fn deploy_one(c: &mut Ctx, opts: &Opts, target: Target) -> Result<()> {
    match target {
        Target::Web => web::deploy(
            c.env,
            c.root,
            opts.project.as_deref(),
            opts.dry_run,
            c.tool,
            c.out,
        ),
        _ => native::deploy(c, target, opts.dry_run),
    }
}

/// Confirms (unless `--yes`/`--dry-run`), then deploys every chosen target in order.
/// With several targets a failing one is reported and the rest still run; the command then fails.
pub fn run(
    opts: &Opts,
    vars: &Vars,
    tool: &dyn Tool,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()> {
    if !opts.path.is_dir() {
        bail!("not a directory: {}", opts.path.display());
    }
    let root = opts
        .path
        .canonicalize()
        .with_context(|| format!("resolving {}", opts.path.display()))?;
    let mut c = Ctx {
        env: opts.env,
        root: &root,
        vars,
        tool,
        out,
        err,
    };
    if !opts.dry_run && !opts.yes {
        let question = match (opts.store_page, opts.choice) {
            (true, Choice::One(t)) => format!(
                "Push the {} store page from {}/stores?",
                t.name(),
                root.display()
            ),
            (true, Choice::All) => {
                format!("Push the all store page from {}/stores?", root.display())
            }
            (false, choice) => format!(
                "Deploy '{}' to '{}' from {}?",
                choice_name(choice),
                opts.env.name(),
                root.display()
            ),
        };
        if !confirm(&question, input, c.out)? {
            bail!("Aborted.");
        }
    }
    if opts.store_page {
        return store_page::push(&mut c, opts.choice, opts.dry_run);
    }
    let targets = opts.choice.targets();
    let mut failed = vec![];
    for target in &targets {
        if let Err(e) = deploy_one(&mut c, opts, *target) {
            if let Choice::One(_) = opts.choice {
                bail!("deploy {}: {e:#}", target.name());
            }
            writeln!(c.err, "deploy {}: {e:#}", target.name())
                .context("reporting a failed target")?;
            failed.push(target.name());
        }
    }
    if failed.is_empty() {
        return Ok(());
    }
    bail!(
        "deploy: {} of {} target(s) failed ({})",
        failed.len(),
        targets.len(),
        failed.join(", ")
    )
}

fn choice_name(choice: Choice) -> &'static str {
    match choice {
        Choice::All => "all",
        Choice::One(t) => t.name(),
    }
}

#[cfg(test)]
mod tests {
    include!("run_tests.rs");
}
