use super::*;
use crate::audit::diag::fake::Fake;
use std::fs;
use std::path::Path;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn proj() -> (tempfile::TempDir, Vec<Entry>) {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("Cargo.toml"), "[package]"));
    ok(fs::write(d.path().join("Cargo.lock"), ""));
    let entries = vec![Entry { name: "ss".into(), path: d.path().to_path_buf() }];
    (d, entries)
}

fn clean() -> Fake {
    Fake::new(&[])
        .answering("git rev-parse --verify HEAD", Ok("abc\n"))
        .answering("git --no-optional-locks status --porcelain", Ok(""))
        .answering("cargo --version", Ok("cargo 1\n"))
}

fn why(r: Result<PathBuf>) -> String {
    r.err().map(|e| format!("{e:#}")).unwrap_or_else(|| panic!("expected an error"))
}

fn canonical(p: &Path) -> PathBuf {
    ok(p.canonicalize())
}

#[test]
fn a_clean_committed_cargo_project_gives_its_canonical_path() {
    let (d, entries) = proj();
    assert_eq!(ok(project(&entries, "ss", &clean())), canonical(d.path()));
}

#[test]
fn an_unknown_project_is_named_with_the_way_to_list_them() {
    let (_d, entries) = proj();
    assert_eq!(why(project(&entries, "zz", &clean())), "Unknown project: zz (run repo list)");
    assert_eq!(why(project(&[], "ss", &clean())), "Unknown project: ss (run repo list)");
}

#[test]
fn a_registered_path_that_is_gone_is_named() {
    let (d, entries) = proj();
    let gone = d.path().join("gone");
    let entries = vec![Entry { name: "ss".into(), path: gone.clone() }, entries[0].clone()];
    assert_eq!(why(project(&entries, "ss", &clean())), format!("registered path is missing: {}", gone.display()));
}

#[test]
fn a_missing_manifest_or_lockfile_is_named() {
    let (d, entries) = proj();
    ok(fs::remove_file(d.path().join("Cargo.lock")));
    let want = format!("Cargo.lock not found in {}", canonical(d.path()).display());
    assert_eq!(why(project(&entries, "ss", &clean())), want);
    ok(fs::remove_file(d.path().join("Cargo.toml")));
    let want = format!("Cargo.toml not found in {}", canonical(d.path()).display());
    assert_eq!(why(project(&entries, "ss", &clean())), want);
}

#[test]
fn a_repository_without_commit_is_refused() {
    let (d, entries) = proj();
    let f = clean().overriding("git rev-parse --verify HEAD", Err("no commit"));
    let want = format!("project is not a committed Git repository: {}", canonical(d.path()).display());
    assert_eq!(why(project(&entries, "ss", &f)), want);
}

#[test]
fn uncommitted_changes_are_refused() {
    let (d, entries) = proj();
    let f = clean().overriding("git --no-optional-locks status --porcelain", Ok(" M a\n"));
    let want = format!("project has uncommitted changes: {}", canonical(d.path()).display());
    assert_eq!(why(project(&entries, "ss", &f)), want);
}

#[test]
fn a_status_that_cannot_be_read_is_not_read_as_clean() {
    let (d, entries) = proj();
    let f = clean().overriding("git --no-optional-locks status", Err("index.lock"));
    let msg = why(project(&entries, "ss", &f));
    assert!(msg.starts_with(&format!("cannot read Git status for {}", canonical(d.path()).display())), "{msg}");
    assert!(msg.contains("index.lock"), "{msg}");
}

#[test]
fn a_missing_cargo_is_said_last() {
    let (_d, entries) = proj();
    let f = clean().overriding("cargo --version", Err("cargo not found"));
    assert_eq!(why(project(&entries, "ss", &f)), "cargo is unavailable; install Rust through dotfiles first");
}

#[test]
fn git_is_asked_in_the_project_directory_and_nothing_is_compiled() {
    let (_d, entries) = proj();
    let f = clean();
    ok(project(&entries, "ss", &f));
    assert!(f.calls.borrow().iter().all(|c| !c.starts_with("cargo install")));
}
