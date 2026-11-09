//! repo — name with actname, scaffold/manage with `dotfiles project`, PRs with gh.
mod delegate;
#[cfg(test)]
mod fakes;
mod naming;
mod pipeline;
mod root;

use actit::github::GhNames;
use actit::process::KagProcess;
use anyhow::Result;
use clap::{Parser, Subcommand};
use naming::Query;
use pipeline::{Dotfiles, NewOpts, Source};

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
        /// Project type, as in `dotfiles project setup` (e.g. package/rust).
        kind: String,
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
    },
    /// Any other command goes to `dotfiles project` (list, status, pr, doctor, …).
    #[command(external_subcommand)]
    Other(Vec<String>),
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Other(args) => delegate::project(&args),
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
        } => {
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
            let name = pipeline::run(&opts, &names, &Dotfiles)?;
            if dry_run {
                println!("{name}");
            }
            Ok(())
        }
    }
}
