use super::*;
use crate::scaffold::native::Quiet;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) {
    let out = ok(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(dir).output());
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit(dir: &Path, file: &str) {
    ok(fs::write(dir.join(file), file));
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", file]);
}

fn mkrepo(dir: &Path) {
    ok(fs::create_dir_all(dir));
    git(dir, &["init", "-q", "-b", "main"]);
    commit(dir, "one");
}

/// A repo whose upstream is a bare repo kept alive by `keep`.
fn tracked(dir: &Path, keep: &mut Vec<tempfile::TempDir>) {
    mkrepo(dir);
    let rem = ok(tempfile::tempdir());
    git(rem.path(), &["init", "-q", "--bare", "-b", "main"]);
    git(dir, &["remote", "add", "origin", &rem.path().display().to_string()]);
    git(dir, &["push", "-q", "-u", "origin", "main"]);
    keep.push(rem);
}

#[derive(Default)]
struct Pusher {
    calls: RefCell<Vec<String>>,
    fail_on: Option<&'static str>,
}

impl Exec for Pusher {
    fn run(&self, prog: &str, args: &[&str], _cwd: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        if self.fail_on.is_some_and(|f| line.contains(f)) {
            anyhow::bail!("remote rejected");
        }
        Ok(String::new())
    }
}

struct World {
    root: PathBuf,
    _d: tempfile::TempDir,
    keep: Vec<tempfile::TempDir>,
}

/// root (ahead), `dirty` (ahead + untracked file), `synced`, `local` (no upstream).
fn world() -> World {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    let mut keep = Vec::new();
    tracked(&root, &mut keep);
    commit(&root, "ahead");
    tracked(&root.join("dirty"), &mut keep);
    commit(&root.join("dirty"), "ahead");
    ok(fs::write(root.join("dirty/untracked"), "x"));
    tracked(&root.join("synced"), &mut keep);
    mkrepo(&root.join("local"));
    // root's own worktree sees the nested repos as untracked: ignore them so root stays clean
    ok(fs::write(root.join(".git/info/exclude"), "dirty/\nsynced/\nlocal/\n"));
    World { root, _d: d, keep }
}

fn go(w: &World, dry_run: bool, yes: bool, input: &str, pusher: &Pusher) -> (Result<()>, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let opts = Opts { dir: w.root.clone(), dry_run, yes };
    let r = run(&opts, &Quiet, pusher, &mut input.as_bytes(), &mut out, &mut err);
    (r, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn dry_run_reports_candidates_and_pushes_nothing() {
    let w = world();
    let p = Pusher::default();
    let (r, out, _) = go(&w, true, false, "", &p);
    assert!(r.is_ok());
    assert!(out.contains("==> 1 repo(s) to push:\n        . (main, 1 commits ahead)\n"), "{out}");
    assert!(out.ends_with("\n==> dry-run: nothing pushed.\n"), "{out}");
    assert!(p.calls.borrow().is_empty());
    drop(w.keep);
}

#[test]
fn the_table_lists_every_state() {
    let w = world();
    let (_, out, _) = go(&w, true, false, "", &Pusher::default());
    for want in ["ahead:1", "dirty", "up-to-date", "no-upstream"] {
        assert!(out.contains(want), "{want}: {out}");
    }
    assert!(out.contains("    repos     : 4 found"), "{out}");
}

#[test]
fn yes_pushes_only_the_clean_repos_that_are_ahead() {
    let w = world();
    let p = Pusher::default();
    let (r, out, _) = go(&w, false, true, "", &p);
    assert!(r.is_ok());
    let calls = p.calls.borrow();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0], format!("git -C {} push", w.root.display()));
    assert!(out.contains("==> git -C . push"), "{out}");
    assert!(out.ends_with("\n==> pushed 1/1 (0 failed)\n"), "{out}");
}

#[test]
fn without_yes_a_yes_answer_pushes() {
    let w = world();
    let p = Pusher::default();
    let (r, out, _) = go(&w, false, false, "y\n", &p);
    assert!(r.is_ok());
    assert!(out.contains("Push 1 repo(s)?"), "{out}");
    assert_eq!(p.calls.borrow().len(), 1);
}

#[test]
fn without_yes_anything_else_pushes_nothing() {
    for input in ["n\n", "\n", ""] {
        let w = world();
        let p = Pusher::default();
        let (r, out, _) = go(&w, false, false, input, &p);
        assert!(r.is_ok(), "{input:?}");
        assert!(out.contains("==> aborted: nothing pushed."), "{input:?}: {out}");
        assert!(p.calls.borrow().is_empty(), "{input:?}");
    }
}

#[test]
fn a_failed_push_is_counted_reported_and_fails_the_run() {
    let w = world();
    let p = Pusher { fail_on: Some("push"), ..Pusher::default() };
    let (r, out, err) = go(&w, false, true, "", &p);
    assert!(out.ends_with("\n==> pushed 0/1 (1 failed)\n"), "{out}");
    assert!(err.contains("push failed for .") && err.contains("remote rejected"), "{err}");
    assert!(r.err().is_some_and(|e| e.to_string().contains("1 push(es) failed")));
}

#[test]
fn nothing_to_push_says_so_and_asks_nothing() {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    let mut keep = Vec::new();
    tracked(&root, &mut keep);
    let w = World { root, _d: d, keep };
    let (r, out, _) = go(&w, false, false, "", &Pusher::default());
    assert!(r.is_ok());
    assert!(out.ends_with("\n==> nothing to push (no repo has unpushed commits on a clean tree).\n"), "{out}");
    assert!(!out.contains("Push 0"), "{out}");
}

#[test]
fn no_repository_under_the_directory_fails_loudly() {
    let d = ok(tempfile::tempdir());
    let w = World { root: ok(d.path().canonicalize()), _d: d, keep: Vec::new() };
    let (r, out, _) = go(&w, true, true, "", &Pusher::default());
    assert!(r.err().is_some_and(|e| e.to_string().starts_with("No git repos found under ")));
    assert_eq!(out, "");
}

#[test]
fn a_missing_directory_fails_loudly() {
    let w = World { root: PathBuf::from("/nonexistent/zz"), _d: ok(tempfile::tempdir()), keep: Vec::new() };
    let (r, _, _) = go(&w, true, true, "", &Pusher::default());
    assert!(r.err().is_some_and(|e| format!("{e:#}").contains("no such directory")));
}

#[test]
fn an_error_state_is_never_pushed_and_is_warned_about() {
    struct BrokenStatus;
    impl Exec for BrokenStatus {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.contains(&"status") {
                anyhow::bail!("status exploded");
            }
            Quiet.run(p, a, c)
        }
    }
    let w = world();
    let p = Pusher::default();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let opts = Opts { dir: w.root.clone(), dry_run: false, yes: true };
    ok(run(&opts, &BrokenStatus, &p, &mut "".as_bytes(), &mut out, &mut err));
    assert!(p.calls.borrow().is_empty());
    assert!(String::from_utf8_lossy(&err).contains("status exploded"), "{}", String::from_utf8_lossy(&err));
}
