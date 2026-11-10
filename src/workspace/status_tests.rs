use super::*;
use crate::scaffold::native::Quiet;
use anyhow::Result;
use std::fs;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) {
    let o = ok(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(dir).output());
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

fn commit(dir: &Path, file: &str) {
    ok(fs::write(dir.join(file), file));
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", file]);
}

struct Fx {
    _d: tempfile::TempDir,
    work: std::path::PathBuf,
}

/// A repo with a bare origin and an upstream, level with it.
fn level() -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let (work, remote) = (base.join("work"), base.join("remote.git"));
    ok(fs::create_dir_all(&work));
    ok(fs::create_dir_all(&remote));
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["init", "-q", "-b", "main"]);
    commit(&work, "one");
    git(&work, &["remote", "add", "origin", &remote.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    Fx { _d: d, work }
}

fn state(dir: &Path) -> State {
    classify(&Quiet, dir)
}

#[test]
fn words_match_the_bash_version() {
    let words = [
        (State::Missing, "missing"),
        (State::NotGit, "not-git"),
        (State::Detached, "detached"),
        (State::NoRemote, "no-remote"),
        (State::NoUpstream, "no-upstream"),
        (State::UpToDate, "up-to-date"),
        (State::Ahead(3), "ahead:3"),
        (State::Error("boom".into()), "error: boom"),
    ];
    for (s, w) in words {
        assert_eq!(s.to_string(), w);
    }
}

#[test]
fn a_path_that_does_not_exist_is_missing() {
    let f = level();
    assert_eq!(state(&f.work.join("nope")), State::Missing);
}

#[test]
fn a_plain_directory_is_not_git() {
    let d = ok(tempfile::tempdir());
    assert_eq!(state(d.path()), State::NotGit);
}

#[test]
fn a_repo_without_a_remote_is_no_remote() {
    let d = ok(tempfile::tempdir());
    git(d.path(), &["init", "-q", "-b", "main"]);
    commit(d.path(), "a");
    assert_eq!(state(d.path()), State::NoRemote);
}

#[test]
fn a_branch_without_upstream_is_no_upstream() {
    let f = level();
    git(&f.work, &["checkout", "-q", "-b", "topic"]);
    assert_eq!(state(&f.work), State::NoUpstream);
}

#[test]
fn a_detached_head_is_detached() {
    let f = level();
    git(&f.work, &["checkout", "-q", "--detach"]);
    assert_eq!(state(&f.work), State::Detached);
}

#[test]
fn level_with_the_upstream_is_up_to_date() {
    assert_eq!(state(&level().work), State::UpToDate);
}

#[test]
fn commits_ahead_are_counted() {
    let f = level();
    commit(&f.work, "two");
    commit(&f.work, "three");
    assert_eq!(state(&f.work), State::Ahead(2));
}

#[test]
fn uncommitted_changes_do_not_change_the_state() {
    let f = level();
    commit(&f.work, "two");
    ok(fs::write(f.work.join("wip"), "x"));
    ok(fs::write(f.work.join("one"), "edited"));
    assert_eq!(state(&f.work), State::Ahead(1));
}

#[test]
fn commits_behind_only_are_up_to_date() {
    let f = level();
    git(&f.work, &["reset", "-q", "--hard", "HEAD"]);
    commit(&f.work, "two");
    git(&f.work, &["push", "-q"]);
    git(&f.work, &["reset", "-q", "--hard", "HEAD~1"]);
    assert_eq!(state(&f.work), State::UpToDate);
}

/// Fails the commands starting with `bad`; the rest go to real git.
struct Failing(&'static str);

impl Exec for Failing {
    fn run(&self, prog: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
        if args.first() == Some(&self.0) || args.contains(&self.0) {
            anyhow::bail!("{} failed", self.0);
        }
        Quiet.run(prog, args, cwd)
    }
}

#[test]
fn a_failing_remote_listing_is_an_error_not_no_remote() {
    let f = level();
    assert!(matches!(classify(&Failing("remote"), &f.work), State::Error(w) if w.contains("remote")));
}

#[test]
fn a_failing_rev_list_is_an_error_not_up_to_date() {
    let f = level();
    commit(&f.work, "two");
    assert!(matches!(classify(&Failing("rev-list"), &f.work), State::Error(w) if w.contains("rev-list")));
}

#[test]
fn a_count_that_is_not_a_number_is_an_error() {
    struct Garbage;
    impl Exec for Garbage {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.first() == Some(&"rev-list") {
                return Ok("many\n".into());
            }
            Quiet.run(p, a, c)
        }
    }
    let f = level();
    assert!(matches!(classify(&Garbage, &f.work), State::Error(w) if w.contains("many")));
}
