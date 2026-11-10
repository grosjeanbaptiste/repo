use super::*;
use crate::fakes::FakeForge;
use actautoma::forge::Mergeable::{self, Conflicting, Unknown};
use std::cell::RefCell;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let v: Vec<(String, String)> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    move |k| v.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
}

#[test]
fn the_wait_defaults_to_ten_tries_three_seconds_apart() {
    assert_eq!(wait_from_env(env(&[])).ok(), Some(Wait { tries: 10, secs: 3 }));
}

#[test]
fn the_wait_is_read_from_the_environment() {
    let e = env(&[("DOTFILES_PR_MERGEABLE_TRIES", "2"), ("DOTFILES_PR_MERGEABLE_WAIT", "0")]);
    assert_eq!(wait_from_env(e).ok(), Some(Wait { tries: 2, secs: 0 }));
}

#[test]
fn an_invalid_wait_variable_fails_loudly_naming_it() {
    for (k, v) in [("DOTFILES_PR_MERGEABLE_TRIES", "many"), ("DOTFILES_PR_MERGEABLE_WAIT", "-1"), ("DOTFILES_PR_MERGEABLE_WAIT", "")] {
        let why = wait_from_env(env(&[(k, v)])).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(why.contains(k), "{k}={v:?}: {why}");
    }
}

/// Scripted `git`: records every call; a command starting with `fail` errors.
struct Git {
    calls: RefCell<Vec<String>>,
    fail: Option<&'static str>,
}

impl Git {
    fn new() -> Self {
        Self { calls: RefCell::new(vec![]), fail: None }
    }
    fn failing(mut self, prefix: &'static str) -> Self {
        self.fail = Some(prefix);
        self
    }
    fn log(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Exec for Git {
    fn run(&self, prog: &str, args: &[&str], _: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        if self.fail.is_some_and(|p| line.starts_with(p)) {
            anyhow::bail!("{line} failed");
        }
        Ok(String::new())
    }
}

struct Run {
    result: Result<()>,
    out: String,
    err: String,
    sleeps: Vec<u64>,
}

fn go(opts: Opts, wait: Wait, forge: &FakeForge, git: &Git, input: &str) -> Run {
    let (mut out, mut err, sleeps) = (Vec::new(), Vec::new(), RefCell::new(vec![]));
    let result = super::run(&opts, &wait, forge, git, git, &|s| sleeps.borrow_mut().push(s), Path::new("."), &mut input.as_bytes(), &mut out, &mut err);
    Run {
        result,
        out: String::from_utf8_lossy(&out).into_owned(),
        err: String::from_utf8_lossy(&err).into_owned(),
        sleeps: sleeps.into_inner(),
    }
}

fn yes(n: Option<u64>) -> Opts {
    Opts { number: n, yes: true }
}

fn merged(forge: &FakeForge) -> bool {
    forge.log().iter().any(|l| l.starts_with("merge"))
}

const QUICK: Wait = Wait { tries: 10, secs: 3 };

#[test]
fn a_mergeable_pr_is_reported_merged_and_the_branch_deleted() {
    let f = FakeForge::pr("Add x", Mergeable::Mergeable);
    let r = go(yes(Some(8)), QUICK, &f, &Git::new(), "");
    assert!(r.result.is_ok(), "{:?} {}", r.result, r.err);
    assert!(r.out.contains("\nPR #8\n  Title: Add x\n  Source: feature\n  Target: main\n  Author: ana\n  Mergeable: MERGEABLE\n  Review: APPROVED\n  Merge state: CLEAN\n\n"), "{}", r.out);
    assert_eq!(f.log().last().map(String::as_str), Some("merge 8"));
    assert!(r.sleeps.is_empty());
}

#[test]
fn the_local_branches_are_updated_after_the_merge() {
    let f = FakeForge::pr("Add x", Mergeable::Mergeable);
    let g = Git::new();
    let r = go(yes(Some(8)), QUICK, &f, &g, "");
    assert!(r.out.contains("Updating local branches..."), "{}", r.out);
    assert!(g.log().iter().any(|l| l == "git fetch -p"), "{:?}", g.log());
}

#[test]
fn a_title_with_a_pipe_survives() {
    let f = FakeForge::pr("a | b", Mergeable::Mergeable);
    let r = go(yes(Some(8)), QUICK, &f, &Git::new(), "");
    assert!(r.out.contains("  Title: a | b\n"), "{}", r.out);
}

#[test]
fn without_a_number_the_platform_picks_the_pr_of_the_current_branch() {
    let f = FakeForge::pr("Add x", Mergeable::Mergeable);
    go(yes(None), QUICK, &f, &Git::new(), "");
    assert_eq!(f.log(), ["view", "merge 8"]);
}

#[test]
fn unknown_is_asked_again_until_the_platform_answers() {
    let f = FakeForge::pr("Add x", Unknown).polling(&[Unknown, Mergeable::Mergeable]);
    let r = go(yes(Some(8)), QUICK, &f, &Git::new(), "");
    assert!(r.result.is_ok(), "{:?} {}", r.result, r.err);
    assert_eq!(r.sleeps, vec![3, 3]);
    assert!(r.err.contains("Mergeability of PR #8 not computed yet by GitHub; asking again in 3s (1/10).\n"), "{}", r.err);
    assert!(r.err.contains("(2/10)."), "{}", r.err);
    assert!(r.out.contains("  Mergeable: MERGEABLE\n"), "{}", r.out);
    assert_eq!(f.log(), ["view 8", "view 8", "view 8", "merge 8"]);
}

#[test]
fn unknown_forever_gives_up_after_the_tries_and_merges_nothing() {
    let f = FakeForge::pr("Add x", Unknown);
    let r = go(yes(Some(8)), Wait { tries: 3, secs: 1 }, &f, &Git::new(), "");
    assert_eq!(r.sleeps, vec![1, 1, 1]);
    assert!(r.err.contains("This pull request is not ready to merge."), "{}", r.err);
    assert!(r.out.contains("  Mergeable: UNKNOWN\n"), "{}", r.out);
    assert!(r.result.is_err());
    assert!(!merged(&f), "{:?}", f.log());
}

#[test]
fn zero_tries_never_waits() {
    let f = FakeForge::pr("Add x", Unknown).polling(&[Mergeable::Mergeable]);
    let r = go(yes(Some(8)), Wait { tries: 0, secs: 3 }, &f, &Git::new(), "");
    assert!(r.sleeps.is_empty());
    assert!(r.result.is_err());
}

#[test]
fn a_conflicting_pr_is_refused_without_waiting() {
    let f = FakeForge::pr("Add x", Conflicting);
    let r = go(yes(Some(8)), QUICK, &f, &Git::new(), "");
    assert!(r.sleeps.is_empty());
    assert!(r.err.contains("This pull request is not ready to merge."), "{}", r.err);
    assert!(r.result.is_err());
    assert!(!merged(&f));
}

#[test]
fn only_y_or_yes_confirms() {
    for ok in ["y\n", "Y\n", "yes\n", "YES\n"] {
        let f = FakeForge::pr("Add x", Mergeable::Mergeable);
        let r = go(Opts { number: Some(8), yes: false }, QUICK, &f, &Git::new(), ok);
        assert!(r.out.contains("Merge this pull request? [y/N] "), "{}", r.out);
        assert!(merged(&f), "{ok:?}");
    }
}

#[test]
fn any_other_answer_cancels_and_succeeds() {
    for no in ["", "n\n", "\n", "maybe\n", "yess\n"] {
        let (f, g) = (FakeForge::pr("Add x", Mergeable::Mergeable), Git::new());
        let r = go(Opts { number: Some(8), yes: false }, QUICK, &f, &g, no);
        assert!(r.result.is_ok(), "{no:?}");
        assert!(r.out.ends_with("Cancelled.\n"), "{}", r.out);
        assert!(!merged(&f), "{no:?}");
        assert!(g.log().is_empty(), "{no:?}");
    }
}

#[test]
fn a_failing_view_fails_before_anything_else() {
    let f = FakeForge::pr("x", Mergeable::Mergeable).failing("view");
    let r = go(yes(Some(8)), QUICK, &f, &Git::new(), "");
    assert!(r.result.is_err());
    assert_eq!(f.log(), ["view 8"]);
}

#[test]
fn a_failing_mergeability_poll_fails_loudly() {
    let f = FakeForge::pr("x", Unknown).failing("poll");
    let r = go(yes(Some(8)), QUICK, &f, &Git::new(), "");
    assert!(r.result.is_err());
    assert!(!merged(&f));
}

#[test]
fn a_failing_merge_fails_and_leaves_the_local_branches_alone() {
    let (f, g) = (FakeForge::pr("x", Mergeable::Mergeable).failing("merge"), Git::new());
    let r = go(yes(Some(8)), QUICK, &f, &g, "");
    assert!(r.result.is_err());
    assert!(g.log().is_empty(), "{:?}", g.log());
}

#[test]
fn a_failing_local_update_fails_the_command_after_the_merge() {
    let (f, g) = (FakeForge::pr("x", Mergeable::Mergeable), Git::new().failing("git fetch"));
    let r = go(yes(Some(8)), QUICK, &f, &g, "");
    assert!(merged(&f));
    assert!(r.err.contains("warning: PR merged; git fetch failed."), "{}", r.err);
    assert!(r.result.is_err());
}
