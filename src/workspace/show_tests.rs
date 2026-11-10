use super::*;
use crate::scaffold::native::Quiet;
use std::fs;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn ws(json: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let d = ok(tempfile::tempdir());
    let dir = ok(d.path().canonicalize());
    let f = dir.join("w.code-workspace");
    ok(fs::write(&f, json));
    (d, f)
}

fn listed(f: &Path, home: Option<&str>) -> String {
    let mut out = Vec::new();
    ok(list(f, home, &mut out));
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn list_prints_a_padded_table() {
    let (_d, f) = ws(r#"{"folders":[{"name":"api","path":"a"},{"path":"b"}]}"#);
    let want = format!("{:<30} PATH\n{:<30} ----\n{:<30} a\n{:<30} b\n", "NAME", "----", "api", "b");
    assert_eq!(listed(&f, None), want);
}

#[test]
fn list_of_an_empty_workspace_says_so_with_the_home_stripped() {
    let (_d, f) = ws("{}");
    let home = f.parent().and_then(Path::parent).map(|p| p.display().to_string()).unwrap_or_default();
    let rel = f.strip_prefix(&home).map(|p| p.display().to_string()).unwrap_or_default();
    assert_eq!(listed(&f, Some(&home)), format!("No folders in {rel}.\n"));
}

#[test]
fn list_without_a_matching_home_shows_the_full_path() {
    let (_d, f) = ws("{}");
    assert_eq!(listed(&f, Some("/elsewhere")), format!("No folders in {}.\n", f.display()));
}

#[test]
fn list_of_a_broken_workspace_fails_loudly_and_prints_nothing() {
    let (_d, f) = ws("not json");
    let mut out = Vec::new();
    assert!(list(&f, None, &mut out).is_err());
    assert!(out.is_empty());
}

#[test]
fn list_of_an_unreadable_file_fails_naming_it() {
    let (d, _f) = ws("{}");
    let gone = d.path().join("gone.code-workspace");
    let why = list(&gone, None, &mut Vec::new()).err().map(|e| format!("{e:#}")).unwrap_or_default();
    assert!(why.contains("gone.code-workspace"), "{why}");
}

#[test]
fn status_gives_one_row_per_folder_with_its_state() {
    let (d, f) = ws(r#"{"folders":[{"name":"plain","path":"plain"},{"path":"absent"}]}"#);
    ok(fs::create_dir(d.path().join("plain")));
    let mut out = Vec::new();
    ok(status(&f, &Quiet, &mut out));
    let want = format!(
        "{:<25} {:<12} PATH\n{:<25} {:<12} ----\n{:<25} {:<12} plain\n{:<25} {:<12} absent\n",
        "FOLDER", "STATUS", "------", "------", "plain", "not-git", "absent", "missing"
    );
    assert_eq!(String::from_utf8_lossy(&out), want);
}

#[test]
fn status_reads_a_repo_state() {
    let (d, f) = ws(r#"{"folders":[{"path":"r"}]}"#);
    let r = d.path().join("r");
    ok(fs::create_dir(&r));
    ok(Command::new("git").args(["init", "-q", "-b", "main"]).current_dir(&r).status());
    let mut out = Vec::new();
    ok(status(&f, &Quiet, &mut out));
    assert!(String::from_utf8_lossy(&out).contains("no-remote"), "{}", String::from_utf8_lossy(&out));
}

#[test]
fn status_of_a_broken_workspace_fails() {
    let (_d, f) = ws("[");
    assert!(status(&f, &Quiet, &mut Vec::new()).is_err());
}
