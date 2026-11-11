//! `repo doctor`: the report on the real `git`, the real platform tool (`gh`, through actautoma) and the clock.
use super::{Ports, date, run};
use crate::audit;
use crate::scaffold::native::Quiet;
use actautoma::{gh::Gh, process::ProcessRunner};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const STALE_DAYS: i64 = 30;

/// The instant `STALE_DAYS` before `now` (seconds since the epoch).
fn cutoff_at(now: i64) -> String {
    date::utc(now - STALE_DAYS * 86_400)
}

fn cutoff() -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("the clock is before 1970")?;
    let secs = i64::try_from(now.as_secs()).context("the clock is out of range")?;
    Ok(cutoff_at(secs))
}

pub fn command(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("doctor: path is not a directory: {}", path.display());
    }
    let root = path
        .canonicalize()
        .with_context(|| format!("resolving {}", path.display()))?;
    let forge = Gh::new(ProcessRunner).in_dir(&root);
    let audit = |out: &mut dyn std::io::Write, err: &mut dyn std::io::Write| {
        audit::command::run_to(&root, out, err)
    };
    let ports = Ports {
        exec: &Quiet,
        forge: &forge,
        cutoff: cutoff()?,
        audit: &audit,
    };
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    run(&root, &ports, &mut out, &mut err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cutoff_is_thirty_days_before_now() {
        assert_eq!(cutoff_at(1_794_268_800), "2026-10-11T00:00:00Z");
        assert_eq!(cutoff_at(1_794_268_800 + 3_723), "2026-10-11T01:02:03Z");
    }

    #[test]
    fn the_real_cutoff_is_in_the_past_and_well_formed() {
        let c = cutoff().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((c.len(), c.ends_with('Z')), (20, true), "{c}");
        assert!(c.as_str() > "2026-01-01T00:00:00Z", "{c}");
    }
}
