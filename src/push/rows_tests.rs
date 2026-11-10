use super::*;
use crate::scaffold::native::Quiet;
use std::fs;
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

/// A repo with a bare upstream, in sync.
fn tracked(dir: &Path, remote_dir: &Path) {
    mkrepo(dir);
    git(remote_dir, &["init", "-q", "--bare", "-b", "main"]);
    git(dir, &["remote", "add", "origin", &remote_dir.display().to_string()]);
    git(dir, &["push", "-q", "-u", "origin", "main"]);
}

fn row(dir: &Path) -> Row {
    ok(classify(&Quiet, dir, "."))
}

#[test]
fn labels_match_the_bash_states() {
    assert_eq!(State::Dirty.label(), "dirty");
    assert_eq!(State::NoUpstream.label(), "no-upstream");
    assert_eq!(State::UpToDate.label(), "up-to-date");
    assert_eq!(State::Ahead(3).label(), "ahead:3");
    assert_eq!(State::Error("x".into()).label(), "error");
}

#[test]
fn no_upstream_is_reported_with_branch_and_a_missing_origin() {
    let d = ok(tempfile::tempdir());
    mkrepo(d.path());
    let r = row(d.path());
    assert_eq!((r.state, r.branch.as_str(), r.remote.as_str()), (State::NoUpstream, "main", "(no origin)"));
}

#[test]
fn a_dirty_tree_wins_over_everything() {
    let (d, rem) = (ok(tempfile::tempdir()), ok(tempfile::tempdir()));
    tracked(d.path(), rem.path());
    commit(d.path(), "two");
    ok(fs::write(d.path().join("untracked"), "x"));
    assert_eq!(row(d.path()).state, State::Dirty);
}

#[test]
fn in_sync_and_ahead_are_told_apart() {
    let (d, rem) = (ok(tempfile::tempdir()), ok(tempfile::tempdir()));
    tracked(d.path(), rem.path());
    assert_eq!(row(d.path()).state, State::UpToDate);
    commit(d.path(), "two");
    commit(d.path(), "three");
    let r = row(d.path());
    assert_eq!(r.state, State::Ahead(2));
    assert_eq!(r.remote, rem.path().display().to_string());
}

#[test]
fn a_detached_head_is_named_detached() {
    let d = ok(tempfile::tempdir());
    mkrepo(d.path());
    git(d.path(), &["checkout", "-q", "--detach"]);
    assert_eq!(row(d.path()).branch, "detached");
}

#[test]
fn an_unexpected_git_failure_is_an_error_state_naming_the_cause() {
    struct Broken;
    impl Exec for Broken {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.contains(&"status") {
                anyhow::bail!("status exploded");
            }
            Quiet.run(p, a, c)
        }
    }
    let d = ok(tempfile::tempdir());
    mkrepo(d.path());
    let r = ok(classify(&Broken, d.path(), "."));
    assert!(matches!(&r.state, State::Error(m) if m.contains("status exploded")), "{:?}", r.state);
}

#[test]
fn collect_finds_nested_repositories_outer_first_then_by_path_length() {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    mkrepo(&root);
    mkrepo(&root.join("zz"));
    mkrepo(&root.join("a/b/c"));
    mkrepo(&root.join("bb"));
    let rows = ok(collect(&Quiet, &root, &mut Vec::new()));
    let rels: Vec<&str> = rows.iter().map(|r| r.rel.as_str()).collect();
    assert_eq!(rels, [".", "bb", "zz", "a/b/c"]);
}

#[test]
fn collect_enters_vendored_directories_like_the_bash_find() {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    mkrepo(&root);
    mkrepo(&root.join("node_modules/dep"));
    let rels: Vec<String> = ok(collect(&Quiet, &root, &mut Vec::new())).into_iter().map(|r| r.rel).collect();
    assert_eq!(rels, [".", "node_modules/dep"]);
}

#[test]
fn a_subdirectory_of_a_repository_still_finds_the_enclosing_repository_first() {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    mkrepo(&root);
    ok(fs::create_dir(root.join("sub")));
    mkrepo(&root.join("sub/inner"));
    let rows = ok(collect(&Quiet, &root.join("sub"), &mut Vec::new()));
    let abs: Vec<PathBuf> = rows.iter().map(|r| r.repo.clone()).collect();
    assert_eq!(abs, [root.clone(), root.join("sub/inner")]);
    assert_eq!(rows[1].rel, "inner");
}

#[test]
fn no_repository_at_all_is_an_error_naming_the_root() {
    let d = ok(tempfile::tempdir());
    let e = collect(&Quiet, d.path(), &mut Vec::new()).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(e.starts_with("No git repos found under "), "{e}");
}

#[test]
fn unreadable_directories_become_warnings() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    mkrepo(&root);
    ok(fs::create_dir(root.join("locked")));
    ok(fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o0)));
    let mut w = Vec::new();
    let rows = ok(collect(&Quiet, &root, &mut w));
    ok(fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o755)));
    assert_eq!(rows.len(), 1);
    assert!(w.iter().any(|m| m.contains("locked")), "{w:?}");
}
