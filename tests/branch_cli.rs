//! `repo rename-branch` end to end on temp git repositories without a remote. No network.
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = ok(Command::new("git").args(args).current_dir(dir).output());
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn repo_on_master() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    git(d.path(), &["init", "-q", "-b", "master"]);
    ok(fs::write(d.path().join("ci.yml"), "branches: [master]\n"));
    git(d.path(), &["add", "."]);
    git(
        d.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "x",
        ],
    );
    d
}

fn rename(dir: &Path, extra: &[&str]) -> Output {
    ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .arg("rename-branch")
        .arg(dir)
        .args(extra)
        .output())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).to_string()
}

#[test]
fn renames_locally_and_warns_about_references() {
    let d = repo_on_master();
    let o = rename(d.path(), &[]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(git(d.path(), &["branch", "--show-current"]).trim(), "main");
    assert!(text(&o.stderr).contains("ci.yml:1:"), "{}", text(&o.stderr));
}

#[test]
fn dry_run_changes_nothing() {
    let d = repo_on_master();
    let o = rename(d.path(), &["--dry-run"]);
    assert!(o.status.success());
    assert!(text(&o.stdout).contains("would run: git branch -m master main"));
    assert_eq!(
        git(d.path(), &["branch", "--show-current"]).trim(),
        "master"
    );
}

#[test]
fn missing_branch_fails_with_its_name() {
    let d = repo_on_master();
    let o = rename(d.path(), &["--from", "nope"]);
    assert!(!o.status.success());
    assert!(text(&o.stderr).contains("\"nope\""), "{}", text(&o.stderr));
}

#[test]
fn invalid_name_is_rejected_before_any_change() {
    let d = repo_on_master();
    let o = rename(d.path(), &["--to", "a b"]);
    assert!(!o.status.success());
    assert!(text(&o.stderr).contains("invalid branch name"));
    assert_eq!(
        git(d.path(), &["branch", "--show-current"]).trim(),
        "master"
    );
}
