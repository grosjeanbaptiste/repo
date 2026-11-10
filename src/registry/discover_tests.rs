use super::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn mkrepo(root: &Path, rel: &str) {
    ok(fs::create_dir_all(root.join(rel).join(".git")));
}

fn names(root: &Path) -> Vec<String> {
    discover(root).repos.iter().map(|p| p.strip_prefix(root).map_or_else(|_| p.display().to_string(), |r| r.display().to_string())).collect()
}

#[test]
fn finds_the_root_and_nested_repositories() {
    let d = ok(tempfile::tempdir());
    mkrepo(d.path(), "");
    mkrepo(d.path(), "a");
    mkrepo(d.path(), "deep/er/b");
    assert_eq!(names(d.path()), ["", "a", "deep/er/b"]);
}

#[test]
fn the_dot_git_directory_is_never_entered() {
    let d = ok(tempfile::tempdir());
    mkrepo(d.path(), "a");
    mkrepo(d.path(), "a/.git/modules/sub");
    assert_eq!(names(d.path()), ["a"]);
}

#[test]
fn a_dot_git_file_is_a_worktree_or_submodule_not_a_repository() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir_all(d.path().join("wt")));
    ok(fs::write(d.path().join("wt/.git"), "gitdir: /elsewhere"));
    assert_eq!(names(d.path()), Vec::<String>::new());
}

#[test]
fn every_pruned_directory_is_skipped_at_any_depth() {
    for p in PRUNED {
        let d = ok(tempfile::tempdir());
        mkrepo(d.path(), "keep");
        mkrepo(d.path(), &format!("{p}/inner"));
        mkrepo(d.path(), &format!("keep/x/{p}/inner"));
        assert_eq!(names(d.path()), ["keep"], "{p}");
    }
}

#[test]
fn a_pruned_name_as_a_file_or_a_suffix_is_not_pruned() {
    let d = ok(tempfile::tempdir());
    mkrepo(d.path(), "my-build/r");
    mkrepo(d.path(), "vendors/r");
    assert_eq!(names(d.path()), ["my-build/r", "vendors/r"]);
}

#[test]
fn a_pruned_root_itself_is_still_scanned() {
    let d = ok(tempfile::tempdir());
    let root = d.path().join("build");
    mkrepo(&root, "r");
    assert_eq!(discover(&root).repos, [root.join("r")]);
}

#[test]
fn symlinks_are_not_followed() {
    let (d, other) = (ok(tempfile::tempdir()), ok(tempfile::tempdir()));
    mkrepo(other.path(), "outside");
    ok(symlink(other.path(), d.path().join("link")));
    assert_eq!(names(d.path()), Vec::<String>::new());
}

#[test]
fn a_missing_root_or_a_file_root_finds_nothing_without_error() {
    assert_eq!(discover(Path::new("/nonexistent/zz")), Found::default());
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("f"), ""));
    assert_eq!(discover(&d.path().join("f")), Found::default());
}

#[test]
fn an_unreadable_directory_is_reported_not_swallowed() {
    let d = ok(tempfile::tempdir());
    mkrepo(d.path(), "ok");
    ok(fs::create_dir(d.path().join("locked")));
    ok(fs::set_permissions(d.path().join("locked"), fs::Permissions::from_mode(0o0)));
    let found = discover(d.path());
    ok(fs::set_permissions(d.path().join("locked"), fs::Permissions::from_mode(0o755)));
    assert_eq!(found.repos, [d.path().join("ok")]);
    assert_eq!(found.unreadable.len(), 1);
    assert!(found.unreadable[0].contains("locked"), "{:?}", found.unreadable);
}

#[test]
fn results_are_sorted_and_unique() {
    let d = ok(tempfile::tempdir());
    for r in ["z", "m", "a"] {
        mkrepo(d.path(), r);
    }
    let v = discover(d.path()).repos;
    let mut sorted = v.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(v, sorted);
}
