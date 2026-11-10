//! `repo pr merge [N] [--yes]` (port of dotfiles `project pr merge`).
use super::after;
use crate::scaffold::setup::Exec;
use anyhow::{Result, anyhow, bail};
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct Opts {
    pub number: Option<u64>,
    pub yes: bool,
}

/// How long to wait for GitHub to compute mergeability.
#[derive(Debug, PartialEq, Eq)]
pub struct Wait {
    pub tries: u32,
    pub secs: u64,
}

pub fn wait_from_env(env: impl Fn(&str) -> Option<String>) -> Result<Wait> {
    fn read<T: std::str::FromStr>(
        env: &impl Fn(&str) -> Option<String>,
        key: &str,
        default: T,
    ) -> Result<T> {
        match env(key) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| anyhow!("{key} must be a non-negative whole number, got {v:?}")),
        }
    }
    Ok(Wait {
        tries: read(&env, "DOTFILES_PR_MERGEABLE_TRIES", 10)?,
        secs: read(&env, "DOTFILES_PR_MERGEABLE_WAIT", 3)?,
    })
}

const FIELDS: &str = "number,title,headRefName,baseRefName,author,mergeable,reviewDecision";
/// Unit separator: cannot occur in a title, unlike the `|` of the bash version.
const JQ: &str = r#"[.number,.title,.headRefName,.baseRefName,.author.login,.mergeable,.reviewDecision]|map(tostring)|join("\u001f")"#;

struct Pr {
    number: String,
    title: String,
    head: String,
    base: String,
    author: String,
    mergeable: String,
    review: String,
}

fn view(gh: &impl Exec, number: Option<u64>, dir: &Path) -> Result<Pr> {
    let n = number.map(|n| n.to_string());
    let mut args = vec!["pr", "view"];
    args.extend(n.as_deref());
    args.extend(["--json", FIELDS, "--jq", JQ]);
    let raw = gh.run("gh", &args, Some(dir))?;
    let f: Vec<&str> = raw.trim_end_matches('\n').split('\u{1f}').collect();
    let [number, title, head, base, author, mergeable, review] = f[..] else {
        bail!("unexpected answer from `gh pr view`: {raw:?}");
    };
    let own = |s: &str| s.to_string();
    Ok(Pr {
        number: own(number),
        title: own(title),
        head: own(head),
        base: own(base),
        author: own(author),
        mergeable: own(mergeable),
        review: own(review),
    })
}

/// GitHub computes mergeability lazily and answers UNKNOWN just after a push.
fn await_mergeable(
    gh: &impl Exec,
    pr: &Pr,
    wait: &Wait,
    sleep: &dyn Fn(u64),
    dir: &Path,
    err: &mut impl Write,
) -> Result<String> {
    let mut mergeable = pr.mergeable.clone();
    let mut try_ = 0;
    while mergeable == "UNKNOWN" && try_ < wait.tries {
        try_ += 1;
        writeln!(
            err,
            "Mergeability of PR #{} not computed yet by GitHub; asking again in {}s ({try_}/{}).",
            pr.number, wait.secs, wait.tries
        )?;
        sleep(wait.secs);
        let args = [
            "pr",
            "view",
            &pr.number,
            "--json",
            "mergeable",
            "--template",
            "{{.mergeable}}",
        ];
        mergeable = gh.run("gh", &args, Some(dir))?.trim().to_string();
    }
    Ok(mergeable)
}

fn confirmed(opts: &Opts, input: &mut impl BufRead, out: &mut impl Write) -> Result<bool> {
    if opts.yes {
        return Ok(true);
    }
    write!(out, "Merge this pull request? [y/N] ")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?; // end of input leaves it empty: no answer is a no
    Ok(matches!(line.trim(), "y" | "Y" | "yes" | "YES"))
}

#[allow(clippy::too_many_arguments)] // seams: probe/act/sleep/dir/streams, each a distinct port
pub fn run(
    opts: &Opts,
    wait: &Wait,
    probe: &impl Exec,
    act: &impl Exec,
    sleep: &dyn Fn(u64),
    dir: &Path,
    input: &mut impl BufRead,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let pr = view(probe, opts.number, dir)?;
    let mergeable = await_mergeable(probe, &pr, wait, sleep, dir, err)?;
    let args = [
        "pr",
        "view",
        &pr.number,
        "--json",
        "mergeStateStatus",
        "--template",
        "{{.mergeStateStatus}}",
    ];
    let state = probe.run("gh", &args, Some(dir))?;
    writeln!(
        out,
        "\nPR #{}\n  Title: {}\n  Source: {}\n  Target: {}\n  Author: {}",
        pr.number, pr.title, pr.head, pr.base, pr.author
    )?;
    writeln!(
        out,
        "  Mergeable: {mergeable}\n  Review: {}\n  Merge state: {}\n",
        pr.review,
        state.trim()
    )?;
    if mergeable != "MERGEABLE" {
        writeln!(err, "This pull request is not ready to merge.")?;
        bail!("PR #{} is not mergeable ({mergeable})", pr.number);
    }
    if !confirmed(opts, input, out)? {
        writeln!(out, "Cancelled.")?;
        return Ok(());
    }
    act.run(
        "gh",
        &["pr", "merge", &pr.number, "--merge", "--delete-branch"],
        Some(dir),
    )?;
    after::update(probe, act, dir, &pr.base, &pr.head, out, err)
}

#[cfg(test)]
mod tests {
    include!("merge_tests.rs");
}
