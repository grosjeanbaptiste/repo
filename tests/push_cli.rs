//! `repo push` end to end against real repositories and a bare remote. No network.
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = ok(Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .output());
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

struct Fx {
    _d: tempfile::TempDir,
    repo: std::path::PathBuf,
    remote: std::path::PathBuf,
}

/// A repo one commit ahead of its bare remote.
fn ahead() -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let (repo, remote) = (base.join("work"), base.join("remote.git"));
    ok(fs::create_dir_all(&repo));
    ok(fs::create_dir_all(&remote));
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&repo, &["init", "-q", "-b", "main"]);
    ok(fs::write(repo.join("a"), "a"));
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    git(
        &repo,
        &["remote", "add", "origin", &remote.display().to_string()],
    );
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    ok(fs::write(repo.join("b"), "b"));
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "two"]);
    Fx {
        _d: d,
        repo,
        remote,
    }
}

fn push(dir: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .arg("push")
        .arg("--dir")
        .arg(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn());
    let mut si = child.stdin.take().unwrap_or_else(|| panic!("no stdin"));
    let _ = si.write_all(stdin.as_bytes()); // the child may exit before reading
    drop(si);
    ok(child.wait_with_output())
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

#[test]
fn dry_run_shows_the_plan_and_leaves_the_remote_alone() {
    let f = ahead();
    let o = push(&f.repo, &["--dry-run"], "");
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).contains("ahead:1") && out(&o).contains("dry-run: nothing pushed."),
        "{}",
        out(&o)
    );
    assert_eq!(git(&f.remote, &["rev-list", "--count", "main"]), "1");
}

#[test]
fn yes_really_pushes() {
    let f = ahead();
    let o = push(&f.repo, &["--yes"], "");
    assert!(o.status.success(), "{}", err(&o));
    assert!(out(&o).contains("==> pushed 1/1 (0 failed)"), "{}", out(&o));
    assert_eq!(git(&f.remote, &["rev-list", "--count", "main"]), "2");
}

#[test]
fn an_empty_stdin_without_yes_never_pushes() {
    let f = ahead();
    let o = push(&f.repo, &[], "");
    assert!(o.status.success(), "{}", err(&o));
    assert!(out(&o).contains("aborted: nothing pushed."), "{}", out(&o));
    assert_eq!(git(&f.remote, &["rev-list", "--count", "main"]), "1");
}

#[test]
fn answering_y_on_stdin_pushes() {
    let f = ahead();
    let o = push(&f.repo, &[], "y\n");
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(git(&f.remote, &["rev-list", "--count", "main"]), "2");
}

#[test]
fn a_rejected_push_fails_the_command_and_names_the_repository() {
    let f = ahead();
    git(
        &f.repo,
        &["remote", "set-url", "origin", "/nonexistent/remote.git"],
    );
    let o = push(&f.repo, &["--yes"], "");
    assert_eq!(o.status.code(), Some(1));
    assert!(out(&o).contains("(1 failed)"), "{}", out(&o));
    assert!(err(&o).contains("push failed for ."), "{}", err(&o));
}

#[test]
fn a_directory_without_repositories_fails_loudly() {
    let d = ok(tempfile::tempdir());
    let o = push(d.path(), &["--dry-run"], "");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("No git repos found under"), "{}", err(&o));
}

#[test]
fn a_missing_directory_fails_loudly() {
    let o = push(Path::new("/nonexistent/zz"), &["--dry-run"], "");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("no such directory"), "{}", err(&o));
}

#[test]
fn the_default_directory_is_the_current_one() {
    let f = ahead();
    let o = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(["push", "--dry-run"])
        .current_dir(&f.repo)
        .output());
    assert!(o.status.success(), "{}", err(&o));
    assert!(out(&o).contains("ahead:1"), "{}", out(&o));
}
