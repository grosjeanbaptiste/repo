use super::*;
use crate::audit::diag::fake::Fake;
use crate::fakes::FakeForge;
use actautoma::forge::{CiRun, Mergeable, OpenPr};

const CUTOFF: &str = "2026-10-11T00:00:00Z";

fn row(k: &str, v: &str) -> String {
    format!("    {k:<22} {v}")
}

fn pr(number: u64, mine: bool, at: &str) -> OpenPr {
    OpenPr { number, mine, updated_at: at.into() }
}

fn run() -> CiRun {
    CiRun { workflow: "CI".into(), state: "success".into(), date: "2026-11-09".into() }
}

fn github() -> Fake {
    Fake::new(&[]).answering("git config --get remote.origin.url", Ok("git@github.com:me/ss.git\n"))
}

fn forge() -> FakeForge {
    FakeForge::pr("x", Mergeable::Mergeable).hosting("me/ss", "develop")
}

fn go(forge: &FakeForge, exec: &Fake) -> (Vec<String>, Vec<String>) {
    let mut warnings = vec![];
    let lines = lines(forge, exec, Path::new("/w/ss"), CUTOFF, &mut warnings);
    (lines, warnings)
}

#[test]
fn a_missing_platform_tool_is_said_and_nothing_else_is_asked() {
    let f = forge().failing("available");
    let (got, warnings) = go(&f, &github());
    assert_eq!(got.len(), 1);
    assert!(got[0].contains("not installed") || got[0].contains("unavailable"), "{got:?}");
    assert!(got[0].contains("available failed"), "{got:?}");
    assert_eq!(f.log(), ["available"]);
    assert!(warnings.is_empty());
}

#[test]
fn no_origin_remote_is_an_answer() {
    let e = Fake::new(&[]).answering("git config --get remote.origin.url", Err("exit 1"));
    let f = forge();
    assert_eq!(go(&f, &e).0, ["    (no origin remote configured)"]);
    assert_eq!(f.log(), ["available"]);
    let e = Fake::new(&[]).answering("git config --get remote.origin.url", Ok("\n"));
    assert_eq!(go(&forge(), &e).0, ["    (no origin remote configured)"]);
}

#[test]
fn another_host_is_named_and_not_queried() {
    let e = Fake::new(&[]).answering("git config --get remote.origin.url", Ok("git@gitlab.com:me/ss.git\n"));
    let f = forge();
    assert_eq!(go(&f, &e).0, ["    (remote is git@gitlab.com:me/ss.git — not GitHub)"]);
    assert_eq!(f.log(), ["available"]);
}

#[test]
fn open_prs_stale_prs_and_ci_are_reported() {
    let prs = vec![pr(1, true, "2026-11-01T00:00:00Z"), pr(2, false, "2026-09-01T00:00:00Z"), pr(3, false, "2026-10-01T00:00:00Z")];
    let f = forge().open(prs).running(run());
    let (got, warnings) = go(&f, &github());
    assert_eq!(
        got,
        [
            row("Open PRs:", "3 total, 1 mine"),
            row("Stale:", "2 PR(s) inactive >30 days"),
            row("CI on develop:", "CI: success (2026-11-09)"),
        ]
    );
    assert!(warnings.is_empty());
    assert_eq!(f.log(), ["available", "open_prs", "view_repo", "last_run develop"]);
}

#[test]
fn a_pr_updated_exactly_at_the_cutoff_is_not_stale() {
    let f = forge().open(vec![pr(1, true, CUTOFF)]).running(run());
    assert_eq!(go(&f, &github()).0.len(), 2);
}

#[test]
fn no_stale_line_when_nothing_is_stale_and_no_runs_is_said() {
    let f = forge().open(vec![pr(1, true, "2026-11-01T00:00:00Z")]);
    assert_eq!(
        go(&f, &github()).0,
        [row("Open PRs:", "1 total, 1 mine"), row("CI on develop:", "(no runs)")]
    );
}

#[test]
fn no_pr_at_all_counts_zero() {
    let f = forge().running(run());
    assert_eq!(go(&f, &github()).0[0], row("Open PRs:", "0 total, 0 mine"));
}

#[test]
fn a_failing_pr_query_shows_a_question_mark_and_warns() {
    let f = forge().failing("open_prs").running(run());
    let (got, warnings) = go(&f, &github());
    assert_eq!(got[0], row("Open PRs:", "?"));
    assert_eq!(got.len(), 2, "no stale line without a list: {got:?}");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("open PRs") && warnings[0].contains("open_prs failed"), "{warnings:?}");
}

#[test]
fn a_failing_repo_query_falls_back_to_main_and_warns() {
    let f = forge().failing("view_repo").running(run());
    let (got, warnings) = go(&f, &github());
    assert_eq!(got[1], row("CI on main:", "CI: success (2026-11-09)"));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("default branch") && warnings[0].contains("main"), "{warnings:?}");
}

#[test]
fn a_failing_ci_query_shows_a_question_mark_and_warns() {
    let f = forge().failing("last_run");
    let (got, warnings) = go(&f, &github());
    assert_eq!(got[1], row("CI on develop:", "?"));
    assert!(warnings[0].contains("CI") && warnings[0].contains("last_run failed"), "{warnings:?}");
}

#[test]
fn an_unset_origin_is_quiet_but_any_other_git_failure_warns() {
    let unset = Fake::new(&[]).answering("git config --get remote.origin.url", Err("git config exited with exit status: 1: "));
    assert!(go(&forge(), &unset).1.is_empty());
    let broken = Fake::new(&[]).answering("git config --get remote.origin.url", Err("git not found: repo needs it"));
    let (got, warnings) = go(&forge(), &broken);
    assert_eq!(got, ["    (no origin remote configured)"]);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("origin") && warnings[0].contains("git not found"), "{warnings:?}");
}
