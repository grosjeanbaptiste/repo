use super::*;
use crate::scaffold::native::Process;
use std::fs;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output();
    let out = ok(s);
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn repo() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    git(d.path(), &["init", "-q", "-b", "main"]);
    ok(fs::write(d.path().join("f"), "1"));
    git(d.path(), &["add", "-A"]);
    git(d.path(), &["commit", "-q", "-m", "one"]);
    d
}

fn status(d: &Path) -> Status {
    ok(git_status(&Process, d)).unwrap_or_else(|| panic!("not a repo"))
}

#[test]
fn clean_repo_without_remote_or_upstream() {
    let d = repo();
    assert_eq!(
        status(d.path()),
        Status { branch: "main".into(), ahead: None, behind: None, dirty: 0, remote: "none".into() }
    );
}

#[test]
fn untracked_and_modified_files_are_counted() {
    let d = repo();
    ok(fs::write(d.path().join("new"), "x"));
    ok(fs::write(d.path().join("f"), "2"));
    assert_eq!(status(d.path()).dirty, 2);
}

#[test]
fn a_repository_with_no_commit_yet_still_has_a_branch() {
    let d = ok(tempfile::tempdir());
    git(d.path(), &["init", "-q", "-b", "trunk"]);
    assert_eq!(status(d.path()).branch, "trunk");
}

#[test]
fn a_detached_head_is_reported_as_head() {
    let d = repo();
    git(d.path(), &["checkout", "-q", "--detach"]);
    assert_eq!(status(d.path()).branch, "HEAD");
}

#[test]
fn origin_url_is_the_remote() {
    let d = repo();
    git(d.path(), &["remote", "add", "origin", "git@example.com:a/b.git"]);
    assert_eq!(status(d.path()).remote, "git@example.com:a/b.git");
}

#[test]
fn ahead_and_behind_follow_the_upstream() {
    let (remote, clone) = (ok(tempfile::tempdir()), ok(tempfile::tempdir()));
    git(remote.path(), &["init", "-q", "--bare", "-b", "main"]);
    let d = repo();
    git(d.path(), &["remote", "add", "origin", &remote.path().display().to_string()]);
    git(d.path(), &["push", "-q", "-u", "origin", "main"]);
    assert_eq!((status(d.path()).ahead, status(d.path()).behind), (Some(0), Some(0)));
    ok(fs::write(d.path().join("g"), "x"));
    git(d.path(), &["add", "-A"]);
    git(d.path(), &["commit", "-q", "-m", "two"]);
    assert_eq!((status(d.path()).ahead, status(d.path()).behind), (Some(1), Some(0)));
    git(clone.path(), &["clone", "-q", &remote.path().display().to_string(), "."]);
    ok(fs::write(clone.path().join("h"), "x"));
    git(clone.path(), &["add", "-A"]);
    git(clone.path(), &["commit", "-q", "-m", "theirs"]);
    git(clone.path(), &["push", "-q"]);
    git(d.path(), &["fetch", "-q"]);
    assert_eq!((status(d.path()).ahead, status(d.path()).behind), (Some(1), Some(1)));
}

#[test]
fn a_plain_directory_is_not_a_repository() {
    let d = ok(tempfile::tempdir());
    assert_eq!(ok(git_status(&Process, d.path())), None);
    assert_eq!(ok(git_status(&Process, Path::new("/nonexistent/zz"))), None);
}

#[test]
fn a_subdirectory_of_a_repository_counts() {
    let d = repo();
    ok(fs::create_dir(d.path().join("sub")));
    assert_eq!(status(&d.path().join("sub")).branch, "main");
}

#[test]
fn a_missing_git_is_an_error_not_a_non_repository() {
    struct NoGit;
    impl Exec for NoGit {
        fn run(&self, _: &str, _: &[&str], _: Option<&Path>) -> Result<String> {
            anyhow::bail!("git not found: repo needs it")
        }
    }
    assert!(git_status(&NoGit, Path::new("/tmp")).is_err());
}

#[test]
fn one_prints_the_seven_line_report() {
    let d = repo();
    ok(fs::write(d.path().join("n"), "x"));
    let p = d.path().display();
    assert_eq!(
        ok(one(&Process, "demo", d.path())),
        format!("Project: demo\nPath:    {p}\nBranch:  main\nAhead:   -\nBehind:  -\nDirty:   1 file(s)\nRemote:  none\n")
    );
}

#[test]
fn one_on_a_non_repository_fails_with_the_bash_wording() {
    let d = ok(tempfile::tempdir());
    let e = one(&Process, "demo", d.path()).err().map(|e| e.to_string()).unwrap_or_default();
    assert_eq!(e, format!("Project: demo\nPath:    {}\nNot a git repository (or path missing).", d.path().display()));
}

#[test]
fn all_prints_a_table_sorted_skipping_missing_and_non_repos() {
    let (a, b, plain) = (repo(), repo(), ok(tempfile::tempdir()));
    let entry = |n: &str, p: &Path| Entry { name: n.into(), path: p.to_path_buf() };
    let entries = [
        entry("zz", b.path()),
        entry("aa", a.path()),
        entry("gone", Path::new("/nonexistent/x")),
        entry("plain", plain.path()),
    ];
    let want = format!(
        "{:<30} {:<16} {:>6} {:>6} {:>6}  {}\n{:<30} {:<16} {:>6} {:>6} {:>6}  {}\n{:<30} {:<16} {:>6} {:>6} {:>6}  {}\n",
        "NAME", "BRANCH", "AHEAD", "BEHIND", "DIRTY", "REMOTE",
        "aa", "main", "-", "-", 0, "none",
        "zz", "main", "-", "-", 0, "none"
    );
    assert_eq!(ok(all(&Process, &entries, &mut Vec::new())), want);
}

#[test]
fn all_of_nothing_is_just_the_header() {
    let out = ok(all(&Process, &[], &mut Vec::new()));
    assert_eq!(out.lines().count(), 1);
    assert!(out.starts_with("NAME"));
}

#[test]
fn a_bare_repository_has_no_working_tree_so_zero_dirty_files() {
    let d = ok(tempfile::tempdir());
    git(d.path(), &["init", "-q", "--bare", "-b", "main"]);
    assert_eq!(status(d.path()).dirty, 0);
    assert_eq!(status(d.path()).branch, "main");
}

#[test]
fn one_repo_that_cannot_be_read_is_a_warning_not_the_end_of_the_table() {
    struct Flaky;
    impl Exec for Flaky {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.iter().any(|x| x.contains("bad")) && a.contains(&"status") {
                anyhow::bail!("git status exploded");
            }
            Process.run(p, a, c)
        }
    }
    let (good, bad) = (repo(), ok(tempfile::tempdir()));
    let bad_path = bad.path().join("bad");
    ok(fs::create_dir(&bad_path));
    git(&bad_path, &["init", "-q", "-b", "main"]);
    let entries = [
        Entry { name: "bad".into(), path: bad_path },
        Entry { name: "good".into(), path: good.path().to_path_buf() },
    ];
    let mut warnings = Vec::new();
    let out = ok(all(&Flaky, &entries, &mut warnings));
    assert!(out.contains("good") && !out.contains("bad "), "{out}");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("bad") && warnings[0].contains("exploded"), "{warnings:?}");
}

#[test]
fn the_table_sorts_case_insensitively_like_the_shell_did() {
    let (a, b, c) = (repo(), repo(), repo());
    let e = |n: &str, d: &tempfile::TempDir| Entry { name: n.into(), path: d.path().to_path_buf() };
    let out = ok(all(&Process, &[e("beta", &a), e("Gamma", &b), e("alpha", &c)], &mut Vec::new()));
    let order: Vec<&str> = out.lines().skip(1).map(|l| l.split_whitespace().next().unwrap_or("")).collect();
    assert_eq!(order, ["alpha", "beta", "Gamma"]);
}
