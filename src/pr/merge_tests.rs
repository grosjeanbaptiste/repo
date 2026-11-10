use super::*;
use std::cell::{Cell, RefCell};

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

const SEP: char = '\u{1f}';

/// Scripted `gh`/`git`: `gh pr view N --json mergeable` pops from `mergeable`.
struct Fake {
    calls: RefCell<Vec<String>>,
    first: String,
    mergeable: RefCell<Vec<&'static str>>,
    state: &'static str,
    fail: Option<&'static str>,
}

fn pr(title: &str, mergeable: &str) -> String {
    ["8", title, "feature", "main", "ana", mergeable, "APPROVED"].join(&SEP.to_string())
}

impl Fake {
    fn new(first: String, later: &[&'static str]) -> Self {
        Self { calls: RefCell::new(vec![]), first, mergeable: RefCell::new(later.iter().rev().copied().collect()), state: "CLEAN", fail: None }
    }
    fn failing(mut self, prefix: &'static str) -> Self {
        self.fail = Some(prefix);
        self
    }
    fn log(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Exec for Fake {
    fn run(&self, prog: &str, args: &[&str], _: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        if self.fail.is_some_and(|p| line.starts_with(p)) {
            anyhow::bail!("{line} failed");
        }
        Ok(match args {
            ["pr", "view", .., "--jq", _] => self.first.clone(),
            ["pr", "view", _, "--json", "mergeable", ..] => self.mergeable.borrow_mut().pop().unwrap_or("UNKNOWN").to_string(),
            ["pr", "view", _, "--json", "mergeStateStatus", ..] => self.state.to_string(),
            _ => String::new(),
        })
    }
}

struct Run {
    result: Result<()>,
    out: String,
    err: String,
    gh: Vec<String>,
    sleeps: Vec<u64>,
}

fn go(opts: Opts, wait: Wait, fake: &Fake, input: &str) -> Run {
    let (mut out, mut err, sleeps) = (Vec::new(), Vec::new(), RefCell::new(vec![]));
    let result = super::run(&opts, &wait, fake, fake, &|s| sleeps.borrow_mut().push(s), Path::new("."), &mut input.as_bytes(), &mut out, &mut err);
    let log = fake.log();
    Run {
        result,
        out: String::from_utf8_lossy(&out).into_owned(),
        err: String::from_utf8_lossy(&err).into_owned(),
        gh: log.into_iter().filter(|l| l.starts_with("gh ")).collect(),
        sleeps: sleeps.into_inner(),
    }
}

fn yes(n: Option<u64>) -> Opts {
    Opts { number: n, yes: true }
}

const QUICK: Wait = Wait { tries: 10, secs: 3 };

#[test]
fn a_mergeable_pr_is_reported_merged_with_a_merge_commit_and_the_branch_deleted() {
    let f = Fake::new(pr("Add x", "MERGEABLE"), &[]);
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.result.is_ok(), "{:?} {}", r.result, r.err);
    assert!(r.out.contains("\nPR #8\n  Title: Add x\n  Source: feature\n  Target: main\n  Author: ana\n  Mergeable: MERGEABLE\n  Review: APPROVED\n  Merge state: CLEAN\n\n"), "{}", r.out);
    assert_eq!(r.gh.last().map(String::as_str), Some("gh pr merge 8 --merge --delete-branch"));
    assert!(r.sleeps.is_empty());
}

#[test]
fn the_local_branches_are_updated_after_the_merge() {
    let f = Fake::new(pr("Add x", "MERGEABLE"), &[]);
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.out.contains("Updating local branches..."), "{}", r.out);
    let log = f.log();
    assert!(log.iter().any(|l| l == "git fetch -p"), "{log:?}");
}

#[test]
fn a_title_with_a_pipe_survives() {
    let f = Fake::new(pr("a | b", "MERGEABLE"), &[]);
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.out.contains("  Title: a | b\n"), "{}", r.out);
}

#[test]
fn without_a_number_gh_picks_the_pr_of_the_current_branch() {
    let f = Fake::new(pr("Add x", "MERGEABLE"), &[]);
    let r = go(yes(None), QUICK, &f, "");
    assert!(r.gh[0].starts_with("gh pr view --json "), "{:?}", r.gh);
    assert!(r.gh.contains(&"gh pr merge 8 --merge --delete-branch".to_string()), "{:?}", r.gh);
}

#[test]
fn unknown_is_asked_again_until_github_answers() {
    let f = Fake::new(pr("Add x", "UNKNOWN"), &["UNKNOWN", "MERGEABLE"]);
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.result.is_ok(), "{:?} {}", r.result, r.err);
    assert_eq!(r.sleeps, vec![3, 3]);
    assert!(r.err.contains("Mergeability of PR #8 not computed yet by GitHub; asking again in 3s (1/10).\n"), "{}", r.err);
    assert!(r.err.contains("(2/10)."), "{}", r.err);
    assert!(r.out.contains("  Mergeable: MERGEABLE\n"), "{}", r.out);
}

#[test]
fn unknown_forever_gives_up_after_the_tries_and_merges_nothing() {
    let f = Fake::new(pr("Add x", "UNKNOWN"), &[]);
    let r = go(yes(Some(8)), Wait { tries: 3, secs: 1 }, &f, "");
    assert_eq!(r.sleeps, vec![1, 1, 1]);
    assert!(r.err.contains("This pull request is not ready to merge."), "{}", r.err);
    assert!(r.out.contains("  Mergeable: UNKNOWN\n"), "{}", r.out);
    assert!(r.result.is_err());
    assert!(!r.gh.iter().any(|l| l.starts_with("gh pr merge")), "{:?}", r.gh);
}

#[test]
fn zero_tries_never_waits() {
    let f = Fake::new(pr("Add x", "UNKNOWN"), &["MERGEABLE"]);
    let r = go(yes(Some(8)), Wait { tries: 0, secs: 3 }, &f, "");
    assert!(r.sleeps.is_empty());
    assert!(r.result.is_err());
}

#[test]
fn a_conflicting_pr_is_refused_without_waiting() {
    let f = Fake::new(pr("Add x", "CONFLICTING"), &[]);
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.sleeps.is_empty());
    assert!(r.err.contains("This pull request is not ready to merge."), "{}", r.err);
    assert!(r.result.is_err());
    assert!(!r.gh.iter().any(|l| l.starts_with("gh pr merge")));
}

#[test]
fn only_y_or_yes_confirms() {
    for ok in ["y\n", "Y\n", "yes\n", "YES\n"] {
        let f = Fake::new(pr("Add x", "MERGEABLE"), &[]);
        let r = go(Opts { number: Some(8), yes: false }, QUICK, &f, ok);
        assert!(r.out.contains("Merge this pull request? [y/N] "), "{}", r.out);
        assert!(r.gh.iter().any(|l| l.starts_with("gh pr merge")), "{ok:?}");
    }
}

#[test]
fn any_other_answer_cancels_and_succeeds() {
    for no in ["", "n\n", "\n", "maybe\n", "yess\n"] {
        let f = Fake::new(pr("Add x", "MERGEABLE"), &[]);
        let r = go(Opts { number: Some(8), yes: false }, QUICK, &f, no);
        assert!(r.result.is_ok(), "{no:?}");
        assert!(r.out.ends_with("Cancelled.\n"), "{}", r.out);
        assert!(!r.gh.iter().any(|l| l.starts_with("gh pr merge")), "{no:?}");
        assert!(!f.log().iter().any(|l| l.starts_with("git ")), "{no:?}");
    }
}

#[test]
fn a_failing_view_fails_before_anything_else() {
    let f = Fake::new(pr("x", "MERGEABLE"), &[]).failing("gh pr view");
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.result.is_err());
    assert_eq!(r.gh.len(), 1);
}

#[test]
fn a_failing_mergeability_poll_fails_loudly() {
    let f = Fake::new(pr("x", "UNKNOWN"), &[]).failing("gh pr view 8 --json mergeable");
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.result.is_err());
    assert!(!r.gh.iter().any(|l| l.starts_with("gh pr merge")));
}

#[test]
fn a_failing_merge_fails_and_leaves_the_local_branches_alone() {
    let f = Fake::new(pr("x", "MERGEABLE"), &[]).failing("gh pr merge");
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.result.is_err());
    assert!(!f.log().iter().any(|l| l.starts_with("git ")), "{:?}", f.log());
}

#[test]
fn a_failing_local_update_fails_the_command_after_the_merge() {
    let f = Fake::new(pr("x", "MERGEABLE"), &[]).failing("git fetch");
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.gh.iter().any(|l| l.starts_with("gh pr merge")));
    assert!(r.err.contains("warning: PR merged; git fetch failed."), "{}", r.err);
    assert!(r.result.is_err());
}

#[test]
fn a_malformed_pr_answer_is_an_error_not_a_panic() {
    let f = Fake::new("8\u{1f}only two".into(), &[]);
    let r = go(yes(Some(8)), QUICK, &f, "");
    assert!(r.result.is_err());
    let _ = (Cell::new(0), &r.sleeps);
}
