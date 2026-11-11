use super::*;
use crate::audit::diag::fake::Fake;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(dir) = p.parent() {
        ok(fs::create_dir_all(dir));
    }
    ok(fs::write(p, text));
}

fn row(k: &str, v: &str) -> String {
    format!("    {k:<22} {v}")
}

#[test]
fn a_tree_without_the_word_counts_zero() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "main.rs", "fn main() {}\n");
    assert_eq!(ok(files_with(d.path(), "TODO")), 0);
}

#[test]
fn files_are_counted_once_however_many_hits() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "a.rs", "// TODO a\n// TODO b\n");
    put(d.path(), "src/b.py", "# TODO\n");
    put(d.path(), "c.rs", "// FIXME\n");
    assert_eq!(ok(files_with(d.path(), "TODO")), 2);
    assert_eq!(ok(files_with(d.path(), "FIXME")), 1);
}

#[test]
fn every_source_extension_counts_and_others_do_not() {
    let d = ok(tempfile::tempdir());
    for ext in ["py", "js", "ts", "tsx", "rs", "go", "sh", "dart"] {
        put(d.path(), &format!("f.{ext}"), "TODO");
    }
    for ext in ["md", "txt", "json", "toml", "c"] {
        put(d.path(), &format!("g.{ext}"), "TODO");
    }
    assert_eq!(ok(files_with(d.path(), "TODO")), 8);
}

#[test]
fn build_directories_are_skipped_at_any_depth() {
    let d = ok(tempfile::tempdir());
    for dir in ["node_modules", ".git", "build", "target", "a/node_modules", "a/b/target"] {
        put(d.path(), &format!("{dir}/x.rs"), "TODO");
    }
    put(d.path(), "src/targets/x.rs", "TODO");
    assert_eq!(ok(files_with(d.path(), "TODO")), 1);
}

#[test]
fn binary_files_and_symlinks_are_not_searched() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("bin.rs"), b"TODO\0\x01"));
    put(d.path(), "real/x.rs", "TODO");
    ok(std::os::unix::fs::symlink(d.path().join("real"), d.path().join("link")));
    ok(std::os::unix::fs::symlink(d.path().join("real/x.rs"), d.path().join("y.rs")));
    assert_eq!(ok(files_with(d.path(), "TODO")), 1);
}

#[test]
fn a_missing_root_is_an_error_naming_it() {
    let d = ok(tempfile::tempdir());
    let e = files_with(&d.path().join("nope"), "TODO").err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("nope")), "must name the path");
}

#[test]
fn an_unreadable_file_is_an_error_not_a_zero() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    put(d.path(), "a.rs", "TODO");
    ok(fs::set_permissions(d.path().join("a.rs"), fs::Permissions::from_mode(0o000)));
    let e = files_with(d.path(), "TODO").err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("a.rs")), "must name the file");
}

#[test]
fn a_repository_reports_the_files_touched_this_week_once_each() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".git/HEAD", "");
    put(d.path(), "a.rs", "// TODO\n");
    let f = Fake::new(&[])
        .answering("git rev-parse --verify -q HEAD", Ok("abc\n"))
        .answering("git log --since=7 days ago --name-only --pretty=format:", Ok("a.rs\nb.rs\n\na.rs\n"));
    assert_eq!(
        ok(lines(&f, d.path())),
        [
            row("Files with TODO:", "1"),
            row("Files with FIXME:", "0"),
            row("Touched (last 7d):", "2 file(s)")
        ]
    );
}

#[test]
fn a_tree_without_git_has_no_touched_line() {
    let d = ok(tempfile::tempdir());
    let f = Fake::new(&[]);
    assert_eq!(ok(lines(&f, d.path())).len(), 2);
    assert!(f.calls.borrow().is_empty());
}

#[test]
fn a_repository_without_commits_touched_nothing() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".git/HEAD", "");
    let f = Fake::new(&[]).answering("git rev-parse --verify -q HEAD", Err("no commit"));
    assert_eq!(ok(lines(&f, d.path()))[2], row("Touched (last 7d):", "0 file(s)"));
    assert_eq!(f.calls.borrow().len(), 1);
}

#[test]
fn a_log_failure_in_a_repository_with_commits_is_an_error() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".git/HEAD", "");
    let f = Fake::new(&[])
        .answering("git rev-parse --verify -q HEAD", Ok("abc\n"))
        .answering("git log", Err("corrupt"));
    let e = lines(&f, d.path()).err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("corrupt")), "must name the failure");
}

#[test]
fn a_search_failure_is_an_error() {
    let d = ok(tempfile::tempdir());
    let f = Fake::new(&[]);
    assert!(lines(&f, &d.path().join("gone")).is_err());
}
