use super::*;
use crate::scaffold::native::{Process, Quiet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = ok(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(dir).output());
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn commit(dir: &Path, file: &str) {
    ok(fs::write(dir.join(file), file));
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", file]);
}

struct World {
    _d: tempfile::TempDir,
    work: PathBuf,
    remote: PathBuf,
    base: PathBuf, // the directory holding work/, remote.git, other/
}

/// `work` is on `feature`, whose PR was merged on the remote (main has one more commit).
fn world() -> World {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let (work, remote, helper) = (base.join("work"), base.join("remote.git"), base.join("helper"));
    for p in [&work, &remote, &helper] {
        ok(fs::create_dir_all(p));
    }
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["init", "-q", "-b", "main"]);
    commit(&work, "one");
    git(&work, &["remote", "add", "origin", &remote.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    git(&work, &["checkout", "-q", "-b", "feature"]);
    commit(&work, "feat");
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    // the "merge" happens on the remote, from another clone
    git(&helper, &["clone", "-q", &remote.display().to_string(), "."]);
    git(&helper, &["merge", "-q", "--no-ff", "-m", "merge feature", "origin/feature"]);
    git(&helper, &["push", "-q", "origin", "main"]);
    World { _d: d, work, remote, base }
}

fn update_in(_w: &World, dir: &Path) -> (Result<()>, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = update(&Quiet, &Process, dir, "main", "feature", &mut out, &mut err);
    (r, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

fn branch_exists(dir: &Path, b: &str) -> bool {
    Command::new("git").args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{b}")]).current_dir(dir).status().is_ok_and(|s| s.success())
}

#[test]
fn base_not_held_elsewhere_is_checked_out_pulled_and_the_head_branch_deleted() {
    let w = world();
    let (r, out, err) = update_in(&w, &w.work);
    assert!(r.is_ok(), "{err}");
    assert!(out.contains("Updating local branches..."), "{out}");
    assert_eq!(git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(git(&w.work, &["log", "-1", "--format=%s"]), "merge feature");
    assert!(!branch_exists(&w.work, "feature"));
}

#[test]
fn base_held_by_another_worktree_is_fast_forwarded_there_and_here_is_detached() {
    let w = world();
    let main_wt = w.base.join("main-wt");
    // work moves off main so another worktree can hold it
    git(&w.work, &["worktree", "add", "-q", &main_wt.display().to_string(), "main"]);
    let (r, _, err) = update_in(&w, &w.work);
    assert!(r.is_ok(), "{err}");
    assert_eq!(git(&main_wt, &["log", "-1", "--format=%s"]), "merge feature");
    assert_eq!(git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD", "detached");
    assert!(!branch_exists(&w.work, "feature"));
}

#[test]
fn a_dirty_worktree_holding_the_base_is_left_alone_with_a_warning() {
    let w = world();
    let main_wt = w.base.join("main-wt");
    git(&w.work, &["worktree", "add", "-q", &main_wt.display().to_string(), "main"]);
    ok(fs::write(main_wt.join("scratch"), "wip"));
    let (r, _, err) = update_in(&w, &w.work);
    assert!(err.contains("warning: PR merged; main is not updated: its worktree") && err.contains("has local changes"), "{err}");
    assert!(r.is_err());
    assert_eq!(git(&main_wt, &["log", "-1", "--format=%s"]), "one");
    assert!(main_wt.join("scratch").exists());
}

#[test]
fn a_head_branch_that_cannot_be_deleted_is_reported_not_swallowed() {
    let w = world();
    git(&w.work, &["checkout", "-q", "feature"]);
    commit(&w.work, "unmerged-local-work"); // not on main: `branch -d` refuses
    let (r, _, err) = update_in(&w, &w.work);
    assert!(err.contains("warning: PR merged; local branch feature was not deleted."), "{err}");
    assert!(r.is_err());
    assert!(branch_exists(&w.work, "feature"));
}

#[test]
fn a_head_branch_already_gone_is_not_a_failure() {
    let w = world();
    git(&w.work, &["checkout", "-q", "main"]);
    git(&w.work, &["branch", "-D", "feature"]);
    let (r, _, err) = update_in(&w, &w.work);
    assert!(r.is_ok(), "{err}");
    assert!(!err.contains("not deleted"), "{err}");
}

#[test]
fn a_failing_worktree_lookup_still_says_the_pr_was_merged() {
    struct NoWorktrees;
    impl Exec for NoWorktrees {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.contains(&"worktree") {
                anyhow::bail!("worktree list failed");
            }
            Quiet.run(p, a, c)
        }
    }
    let w = world();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = update(&NoWorktrees, &Process, &w.work, "main", "feature", &mut out, &mut err);
    assert!(r.is_err());
    let err = String::from_utf8_lossy(&err);
    assert!(err.contains("warning: PR merged; the worktrees could not be listed, main was not updated."), "{err}");
    assert_eq!(git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "feature");
}

#[test]
fn a_failing_fetch_says_the_pr_was_merged_and_stops() {
    let w = world();
    git(&w.work, &["remote", "set-url", "origin", "/nonexistent/remote.git"]);
    let (r, _, err) = update_in(&w, &w.work);
    assert!(err.contains("warning: PR merged; git fetch failed."), "{err}");
    assert!(r.is_err());
    assert_eq!(git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "feature");
    let _ = &w.remote;
}

#[test]
fn a_failing_base_checkout_is_reported() {
    let w = world();
    ok(fs::write(w.work.join("one"), "local edit that blocks the checkout"));
    git(&w.work, &["checkout", "-q", "-b", "other"]); // keeps the edit
    ok(fs::write(w.work.join("one"), "conflicting"));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = update(&Quiet, &Process, &w.work, "nonexistent-base", "feature", &mut out, &mut err);
    assert!(r.is_err());
    assert!(String::from_utf8_lossy(&err).contains("warning: PR merged; nonexistent-base was not updated."), "{}", String::from_utf8_lossy(&err));
}

#[test]
fn a_worktree_on_another_branch_does_not_hold_the_base() {
    let w = world();
    let other = w.base.join("other-wt");
    git(&w.work, &["worktree", "add", "-q", "-b", "other", &other.display().to_string(), "main"]);
    let (r, _, err) = update_in(&w, &w.work);
    assert!(r.is_ok(), "{err}");
    assert_eq!(git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(git(&w.work, &["log", "-1", "--format=%s"]), "merge feature");
}

#[test]
fn the_checkout_running_the_update_is_not_its_own_holder() {
    let w = world();
    git(&w.work, &["checkout", "-q", "main"]); // base is checked out right here
    let (r, _, err) = update_in(&w, &w.work);
    assert!(r.is_ok(), "{err}");
    assert_eq!(git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "main", "not detached");
    assert_eq!(git(&w.work, &["log", "-1", "--format=%s"]), "merge feature");
}
