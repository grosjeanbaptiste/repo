use super::*;
use crate::audit::diag::fake::Fake;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn repo() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join(".git")));
    d
}

fn row(k: &str, v: &str) -> String {
    format!("    {k:<22} {v}")
}

fn base() -> Fake {
    Fake::new(&[])
        .answering("git rev-parse --abbrev-ref HEAD", Ok("main\n"))
        .answering("git --no-optional-locks status --porcelain", Ok(" M a\n?? b\n"))
        .answering("git log -1 --format=%h %s", Ok("abc123 Fix it\n"))
        .answering("git log -1 --format=%ar", Ok("3 days ago\n"))
}

fn tracking(f: Fake) -> Fake {
    f.answering("git rev-parse --abbrev-ref --symbolic-full-name @{u}", Ok("origin/main\n"))
        .answering("git rev-list --count origin/main..HEAD", Ok("2\n"))
        .answering("git rev-list --count HEAD..origin/main", Ok("1\n"))
}

#[test]
fn a_directory_that_is_not_a_repository_says_so() {
    let d = ok(tempfile::tempdir());
    let f = Fake::new(&[]);
    assert_eq!(ok(lines(&f, d.path())), ["    (not a git repo)"]);
    assert!(f.calls.borrow().is_empty());
}

#[test]
fn a_tracking_repository_reports_everything() {
    let d = repo();
    let got = ok(lines(&tracking(base()), d.path()));
    assert_eq!(
        got,
        [
            row("Branch:", "main"),
            row("vs origin/main:", "2 ahead, 1 behind"),
            row("Working tree:", "2 uncommitted change(s)"),
            row("Last commit:", "abc123 Fix it — 3 days ago"),
        ]
    );
}

#[test]
fn no_upstream_is_an_answer_and_counts_nothing() {
    let d = repo();
    let f = base().answering("git rev-parse --abbrev-ref --symbolic-full-name @{u}", Err("no upstream"));
    let got = ok(lines(&f, d.path()));
    assert_eq!(got[1], row("Tracking:", "(no upstream)"));
    assert!(f.calls.borrow().iter().all(|c| !c.contains("rev-list")));
}

#[test]
fn an_unborn_branch_is_detached_and_a_repository_without_commit_has_none() {
    let d = repo();
    let f = Fake::new(&[])
        .answering("git rev-parse --abbrev-ref HEAD", Err("unknown revision"))
        .answering("git rev-parse --abbrev-ref --symbolic-full-name", Err("no upstream"))
        .answering("git --no-optional-locks status", Ok(""))
        .answering("git log", Err("no commits"));
    let got = ok(lines(&f, d.path()));
    assert_eq!(got[0], row("Branch:", "(detached)"));
    assert_eq!(got[2], row("Working tree:", "0 uncommitted change(s)"));
    assert_eq!(got[3], row("Last commit:", "(no commits) — ?"));
}

#[test]
fn a_clean_tree_counts_zero_and_blank_lines_are_not_changes() {
    let d = repo();
    let f = base().overriding("git --no-optional-locks status --porcelain", Ok("\n"));
    assert_eq!(ok(lines(&f, d.path()))[2], row("Working tree:", "0 uncommitted change(s)"));
}

#[test]
fn a_counting_failure_is_an_error_not_a_zero() {
    let d = repo();
    let f = tracking(base()).overriding("git rev-list --count origin/main..HEAD", Err("bad object"));
    let e = lines(&f, d.path()).err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("bad object") && m.contains("ahead")), "must name the failure");
}

#[test]
fn a_status_failure_is_an_error_not_a_zero() {
    let d = repo();
    let f = base()
        .answering("git rev-parse --abbrev-ref --symbolic-full-name", Err("none"))
        .overriding("git --no-optional-locks status --porcelain", Err("index.lock"));
    let e = lines(&f, d.path()).err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("index.lock")), "must name the failure");
}

#[test]
fn a_non_numeric_count_is_an_error() {
    let d = repo();
    let f = tracking(base()).overriding("git rev-list --count HEAD..origin/main", Ok("lots\n"));
    assert!(lines(&f, d.path()).is_err());
}
