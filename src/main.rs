//! repo — name with actname, scaffold/manage with `dotfiles project`, PRs through actautoma.
mod audit;
mod branch;
mod delegate;
mod doctor;
#[cfg(test)]
mod fakes;
mod license;
mod naming;
mod pipeline;
mod pr;
mod push;
mod registry;
mod root;
mod scaffold;
mod workspace;

use actit::github::GhNames;
use actit::process::KagProcess;
use anyhow::Result;
use clap::{Parser, Subcommand};
use naming::Query;
use pipeline::{Dotfiles, NewOpts, Source};
use scaffold::apply::Mode;
use scaffold::command;
use scaffold::native::{Native, Process, Quiet};
use scaffold::{plan, prompt};
use std::path::Path;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Name a new repository (actname, or --name), check it, then scaffold it.
    New {
        /// Project type, as in `dotfiles project setup` (e.g. package/rust); asked on stdin when omitted.
        kind: Option<String>,
        /// Use this exact name instead of asking actname; it must be free.
        #[arg(long, conflicts_with_all = ["prefixes", "domain"])]
        name: Option<String>,
        #[arg(long = "prefix", required_unless_present = "name")]
        prefixes: Vec<String>,
        #[arg(long, required_unless_present = "name")]
        domain: Option<String>,
        #[arg(long, default_value_t = 10)]
        max_length: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, default_value = "~/Source")]
        root: String,
        #[arg(long)]
        no_push: bool,
        /// Print the chosen name and stop.
        #[arg(long)]
        dry_run: bool,
        /// Scaffold with `dotfiles project setup` instead of the built-in Rust code.
        #[arg(long)]
        via_dotfiles: bool,
    },
    /// Audit a project against the standard templates.
    Check {
        #[arg(default_value = ".")]
        path: String,
        #[arg(long = "type")]
        kind: Option<String>,
    },
    /// Drop/refresh the standard files in an existing project (skips existing files).
    Apply {
        #[arg(default_value = ".")]
        path: String,
        #[arg(long = "type")]
        kind: Option<String>,
        /// Overwrite files that exist.
        #[arg(long, conflicts_with = "diff")]
        force: bool,
        /// Show what would change; write nothing.
        #[arg(long)]
        diff: bool,
    },
    /// Say which licence a project carries (none, proprietary, open, other).
    License {
        #[arg(default_value = ".")]
        path: String,
    },
    /// Rename a repository's default branch, locally and on GitHub (master -> main).
    RenameBranch {
        #[arg(default_value = ".")]
        path: String,
        #[arg(long, default_value = "master")]
        from: String,
        #[arg(long, default_value = "main")]
        to: String,
        /// Print the steps; change nothing.
        #[arg(long)]
        dry_run: bool,
        /// Also delete the old branch on origin (irreversible).
        #[arg(long)]
        delete_old: bool,
    },
    /// List the registered projects (name and path).
    List,
    /// Find git repositories (default root: $HOME) and refresh the registry.
    Scan { roots: Vec<String> },
    /// Branch, ahead/behind, dirty files and remote for one project, or a table of all.
    Status { name: Option<String> },
    /// Push a repository and every repository nested under it (clean tree, ahead of its upstream).
    Push {
        /// Root to scan.
        #[arg(long, default_value = ".")]
        dir: String,
        /// Report what would be pushed; touch nothing.
        #[arg(long, short = 'n')]
        dry_run: bool,
        /// Push without asking.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Pull requests: create one from the current branch, or merge one and update the local branches.
    Pr {
        #[command(subcommand)]
        sub: PrCmd,
    },
    /// List the env vars and CI secrets a project references, and where each is fulfilled.
    Audit {
        #[arg(default_value = ".")]
        path: String,
    },
    /// One-glance health report: git, open PRs and CI, quality signals, licence, audit.
    Doctor {
        #[arg(default_value = ".")]
        path: String,
    },
    /// Folders of the current `*.code-workspace`: list, add, remove, status, push.
    Workspace {
        #[command(subcommand)]
        sub: WsCmd,
    },
    /// Any other command goes to `dotfiles project` (list, status, pr, doctor, …).
    #[command(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand)]
enum PrCmd {
    /// Open a PR for the current branch; title and description are asked on stdin when not given.
    Create {
        /// Base branch; the only master/main/develop/dev/staging/release candidate is used when omitted.
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body: Option<String>,
    },
    /// Merge a PR (merge commit, branch deleted), then fast-forward the base locally.
    Merge {
        /// PR number; the PR of the current branch when omitted.
        number: Option<u64>,
        /// Merge without asking.
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum WsCmd {
    /// Show the folders.
    List,
    /// Add a folder: adopt ./<name>, or clone or scaffold it first.
    Add {
        name: String,
        /// `git clone` this URL into ./<name>, then add it.
        #[arg(long, conflicts_with = "init")]
        clone: Option<String>,
        /// Scaffold a new project of this type at ./<name>, then add it.
        #[arg(long)]
        init: Option<String>,
    },
    /// Remove a folder from the workspace (the directory stays on disk).
    Remove { name: String },
    /// Git state of every folder.
    Status,
    /// Push every folder that has unpushed commits.
    Push {
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Pr { sub } => match sub {
            PrCmd::Create { base, title, body } => {
                pr::command::run_create(&pr::create::Opts { base, title, body })
            }
            PrCmd::Merge { number, yes } => {
                pr::command::run_merge(&pr::merge::Opts { number, yes })
            }
        },
        Cmd::Workspace { sub } => match sub {
            WsCmd::List => workspace::command::run_list(),
            WsCmd::Status => workspace::command::run_status(),
            WsCmd::Remove { name } => workspace::command::run_remove(&name),
            WsCmd::Push { dry_run } => workspace::command::run_push(dry_run),
            WsCmd::Add { name, clone, init } => {
                let source = match (clone, init) {
                    (Some(url), _) => workspace::edit::Source::Clone(url),
                    (None, Some(kind)) => workspace::edit::Source::Init(kind),
                    (None, None) => workspace::edit::Source::Adopt,
                };
                workspace::command::run_add(&name, &source)
            }
        },
        Cmd::Audit { path } => audit::command::run(Path::new(&path)),
        Cmd::Doctor { path } => doctor::command::command(Path::new(&path)),
        Cmd::Other(args) => delegate::project(&args),
        Cmd::RenameBranch {
            path,
            from,
            to,
            dry_run,
            delete_old,
        } => branch::command::run(&path, &from, &to, dry_run, delete_old),
        Cmd::List => registry::command::run_list(),
        Cmd::Scan { roots } => registry::command::run_scan(&roots),
        Cmd::Status { name } => registry::command::run_status(name.as_deref()),
        Cmd::Push { dir, dry_run, yes } => {
            let opts = push::Opts {
                dir: dir.into(),
                dry_run,
                yes,
            };
            let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
            push::run(
                &opts,
                &Quiet,
                &Process,
                &mut std::io::stdin().lock(),
                &mut out,
                &mut err,
            )
        }
        Cmd::License { path } => {
            println!("{}", license::doctor_line(Path::new(&path))?);
            Ok(())
        }
        Cmd::Check { path, kind } => command::run(Path::new(&path), kind.as_deref(), Mode::Check),
        Cmd::Apply {
            path,
            kind,
            force,
            diff,
        } => {
            let mode = if force {
                Mode::Force
            } else if diff {
                Mode::Diff
            } else {
                Mode::Apply
            };
            command::run(Path::new(&path), kind.as_deref(), mode)
        }
        Cmd::New {
            kind,
            name,
            prefixes,
            domain,
            max_length,
            limit,
            root,
            no_push,
            dry_run,
            via_dotfiles,
        } => {
            let native = Native::from_env()?;
            let kind = match kind {
                Some(kind) => kind,
                None => prompt::ask(
                    &plan::known_types(&native.templates)?,
                    &mut std::io::stdin().lock(),
                    &mut std::io::stderr(),
                )?,
            };
            let source = match name {
                Some(name) => Source::Named(name),
                None => Source::Generated(Query {
                    prefixes,
                    domain: domain.unwrap_or_default(),
                    max_length,
                    limit,
                }),
            };
            let opts = NewOpts {
                kind,
                source,
                root: root::expand(&root, std::env::var("HOME").ok())?,
                no_push,
                dry_run,
            };
            let names = GhNames::new(KagProcess::new("gh"));
            let name = if via_dotfiles {
                pipeline::run(&opts, &names, &Dotfiles)?
            } else {
                pipeline::run(&opts, &names, &native)?
            };
            if dry_run {
                println!("{name}");
            }
            Ok(())
        }
    }
}
