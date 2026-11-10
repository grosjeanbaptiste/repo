//! The real ports of `repo pr`: gh and git on the process, stdin/stdout, the clock.
use super::{create, merge};
use crate::scaffold::native::{Process, Quiet};
use anyhow::Result;
use std::path::Path;
use std::time::Duration;

pub fn run_create(opts: &create::Opts) -> Result<()> {
    let mut out = std::io::stdout().lock();
    create::run(
        opts,
        &Quiet,
        &Process,
        Path::new("."),
        &mut std::io::stdin().lock(),
        &mut out,
    )
}

pub fn run_merge(opts: &merge::Opts) -> Result<()> {
    let wait = merge::wait_from_env(|k| std::env::var(k).ok())?;
    let sleep = |s| std::thread::sleep(Duration::from_secs(s));
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    let input = &mut std::io::stdin().lock();
    merge::run(
        opts,
        &wait,
        &Quiet,
        &Process,
        &sleep,
        Path::new("."),
        input,
        &mut out,
        &mut err,
    )
}
