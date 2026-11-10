use super::*;
use crate::scaffold::native::Quiet;
use std::fs;
use std::path::PathBuf;
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
    dir: PathBuf,
    file: PathBuf,
}

/// A workspace dir; `repo(name, ahead)` adds a repo with a bare remote, `ahead` commits ahead.
fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let dir = ok(d.path().canonicalize());
    Fx { file: dir.join("w.code-workspace"), dir, _d: d }
}

impl Fx {
    fn repo(&self, name: &str, ahead: u32) -> PathBuf {
        let (work, remote) = (self.dir.join(name), self.dir.join(format!("{name}.git")));
        ok(fs::create_dir_all(&work));
        ok(fs::create_dir_all(&remote));
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        git(&work, &["init", "-q", "-b", "main"]);
        commit(&work, "one");
        git(&work, &["remote", "add", "origin", &remote.display().to_string()]);
        git(&work, &["push", "-q", "-u", "origin", "main"]);
        for i in 0..ahead {
            commit(&work, &format!("ahead{i}"));
        }
        work
    }
    fn folders(&self, paths: &[&str]) {
        let list: Vec<String> = paths.iter().map(|p| format!(r#"{{"path":"{p}"}}"#)).collect();
        ok(fs::write(&self.file, format!(r#"{{"folders":[{}]}}"#, list.join(","))));
    }
    fn run(&self, dry: bool) -> (Result<()>, String) {
        let mut out = Vec::new();
        let r = run(&self.file, dry, &Quiet, &Quiet, &mut out);
        (r, String::from_utf8_lossy(&out).into_owned())
    }
    fn remote_head(&self, name: &str) -> String {
        let o = ok(Command::new("git").args(["rev-list", "--count", "main"]).current_dir(self.dir.join(format!("{name}.git"))).output());
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }
}

#[test]
fn a_folder_ahead_is_pushed_and_reported() {
    let f = fx();
    f.repo("api", 2);
    f.folders(&["api"]);
    let (r, out) = f.run(false);
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(out, "==> pushing api (2 commit(s) ahead)...\n    ok\n\n1 pushed, 0 up-to-date/skipped\n");
    assert_eq!(f.remote_head("api"), "3");
}

#[test]
fn dry_run_reports_and_pushes_nothing() {
    let f = fx();
    f.repo("api", 1);
    f.folders(&["api"]);
    let (r, out) = f.run(true);
    assert!(r.is_ok());
    assert_eq!(out, format!("  + would push {:<22} (1 commit(s) ahead)\n\ndry-run: 1 would push, 0 up-to-date/skipped\n", "api"));
    assert_eq!(f.remote_head("api"), "1");
}

#[test]
fn every_other_state_is_listed_and_counted_as_skipped() {
    let f = fx();
    f.repo("level", 0);
    let bare = f.dir.join("bare");
    ok(fs::create_dir(&bare));
    git(&bare, &["init", "-q", "-b", "main"]);
    commit(&bare, "a");
    ok(fs::create_dir(f.dir.join("plain")));
    f.folders(&["level", "bare", "plain", "gone"]);
    let (r, out) = f.run(false);
    assert!(r.is_ok(), "{r:?}");
    let want = format!(
        "  = {:<22} up to date\n  ! {:<22} skipped (no-remote)\n  ! {:<22} skipped (not-git)\n  ! {:<22} skipped (missing)\n\n0 pushed, 4 up-to-date/skipped\n",
        "level", "bare", "plain", "gone"
    );
    assert_eq!(out, want);
}

#[test]
fn two_entries_for_the_same_directory_push_once() {
    let f = fx();
    f.repo("api", 1);
    f.folders(&["api", "./api", "../w-dup/../api"]);
    ok(fs::create_dir_all(f.dir.join("../w-dup")));
    let (r, out) = f.run(false);
    assert!(r.is_ok(), "{r:?}\n{out}");
    assert_eq!(out.matches("==> pushing").count(), 1, "{out}");
}

#[test]
fn a_failing_push_is_indented_counted_and_fails_the_command() {
    let f = fx();
    let work = f.repo("api", 1);
    git(&work, &["remote", "set-url", "origin", "/no/such/remote.git"]);
    f.repo("web", 1);
    f.folders(&["api", "web"]);
    let (r, out) = f.run(false);
    assert!(out.contains("==> pushing api (1 commit(s) ahead)...\n    "), "{out}");
    assert!(out.contains("==> pushing web (1 commit(s) ahead)...\n    ok\n"), "{out}");
    assert!(out.ends_with("\n1 pushed, 0 up-to-date/skipped, 1 failed\n"), "{out}");
    assert!(why(r).contains("1 push(es) failed"));
    assert_eq!(f.remote_head("web"), "2");
}

fn why(r: Result<()>) -> String {
    r.err().map(|e| e.to_string()).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn an_unclassifiable_folder_is_reported_and_fails_the_command() {
    struct Garbage;
    impl Exec for Garbage {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.first() == Some(&"rev-list") {
                return Ok("many\n".into());
            }
            Quiet.run(p, a, c)
        }
    }
    let f = fx();
    f.repo("api", 1);
    f.folders(&["api"]);
    let mut out = Vec::new();
    let r = run(&f.file, false, &Garbage, &Quiet, &mut out);
    let out = String::from_utf8_lossy(&out);
    assert!(out.contains(&format!("  ? {:<22} unknown status: error: ", "api")), "{out}");
    assert!(out.ends_with("\n0 pushed, 0 up-to-date/skipped, 1 unchecked\n"), "{out}");
    assert!(why(r).contains("1 folder(s) could not be checked"));
}

#[test]
fn an_empty_workspace_pushes_nothing_and_succeeds() {
    let f = fx();
    ok(fs::write(&f.file, "{}"));
    let (r, out) = f.run(false);
    assert!(r.is_ok());
    assert_eq!(out, "\n0 pushed, 0 up-to-date/skipped\n");
}

#[test]
fn a_broken_workspace_fails_before_any_push() {
    let f = fx();
    ok(fs::write(&f.file, "nope"));
    assert!(f.run(false).0.is_err());
}
