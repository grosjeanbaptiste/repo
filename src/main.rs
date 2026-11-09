//! repo — name with actname, scaffold/manage with `dotfiles project`, PRs with gh.
mod delegate;
mod naming;

use actit::github::GhNames;
use actit::process::KagProcess;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Name a new repository with actname, then scaffold it (mkdir, git init, GitHub).
    New {
        /// Project type, as in `dotfiles project setup` (e.g. package/rust).
        kind: String,
        #[arg(long = "prefix", required = true)]
        prefixes: Vec<String>,
        #[arg(long)]
        domain: String,
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
    },
    /// Any other command goes to `dotfiles project` (list, status, pr, doctor, …).
    #[command(external_subcommand)]
    Other(Vec<String>),
}

fn expand(root: &str) -> Result<PathBuf> {
    match root.strip_prefix("~/") {
        Some(rest) => Ok(PathBuf::from(std::env::var("HOME")?).join(rest)),
        None => Ok(PathBuf::from(root)),
    }
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Other(args) => delegate::project(&args),
        Cmd::New {
            kind,
            prefixes,
            domain,
            max_length,
            limit,
            root,
            no_push,
            dry_run,
        } => {
            let root = expand(&root)?;
            let names = GhNames::new(KagProcess::new("gh"));
            let q = naming::Query {
                prefixes,
                domain,
                max_length,
                limit,
            };
            let name = naming::first_free(&naming::candidates(&q)?, |n| {
                naming::taken(&root, n, &names)
            })?;
            if dry_run {
                println!("{name}");
                return Ok(());
            }
            delegate::project(&delegate::setup_args(&kind, &name, &root, no_push))
        }
    }
}
