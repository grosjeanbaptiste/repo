//! `repo pr create|merge` end to end: real git repositories and a bare remote, a scripted `gh`.
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
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

fn commit(dir: &Path, file: &str) {
    ok(fs::write(dir.join(file), file));
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", file]);
}

/// `gh` that logs its arguments; `pr merge` merges `feature` on the remote through `helper`.
const GH: &str = r#"#!/bin/sh
echo "$*" >> "$GH_LOG"
case "$*" in
  *"--jq"*) printf '8\037Add x\037feature\037main\037ana\037%s\037APPROVED\n' "${GH_MERGEABLE:-MERGEABLE}" ;;
  *"{{.mergeable}}"*) printf '%s' "${GH_MERGEABLE:-MERGEABLE}" ;;
  *"{{.mergeStateStatus}}"*) printf 'CLEAN' ;;
  "pr merge"*) git -C "$GH_HELPER" merge -q --no-ff -m "merge feature" origin/feature && git -C "$GH_HELPER" push -q origin main ;;
  "pr create"*) echo https://github.com/o/r/pull/9 ;;
esac
"#;

struct Fx {
    _d: tempfile::TempDir,
    work: PathBuf,
    bin: PathBuf,
    log: PathBuf,
    helper: PathBuf,
}

fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let (work, remote, helper, bin) = (
        base.join("work"),
        base.join("remote.git"),
        base.join("helper"),
        base.join("bin"),
    );
    for p in [&work, &remote, &helper, &bin] {
        ok(fs::create_dir_all(p));
    }
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["init", "-q", "-b", "main"]);
    commit(&work, "one");
    git(
        &work,
        &["remote", "add", "origin", &remote.display().to_string()],
    );
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    git(&work, &["checkout", "-q", "-b", "feature"]);
    commit(&work, "feat");
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    git(
        &helper,
        &["clone", "-q", &remote.display().to_string(), "."],
    );
    let gh = bin.join("gh");
    ok(fs::write(&gh, GH));
    ok(fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)));
    let log = base.join("gh.log");
    Fx {
        _d: d,
        work,
        bin,
        log,
        helper,
    }
}

fn path_with(bin: &Path) -> String {
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn repo(f: &Fx, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_repo"));
    cmd.args(args)
        .current_dir(&f.work)
        .env("PATH", path_with(&f.bin))
        .env("GH_LOG", &f.log)
        .env("GH_HELPER", &f.helper)
        .env("DOTFILES_PR_MERGEABLE_WAIT", "0")
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = ok(cmd.spawn());
    ok(child
        .stdin
        .take()
        .map_or(Ok(()), |mut i| i.write_all(stdin.as_bytes())));
    ok(child.wait_with_output())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn gh_log(f: &Fx) -> String {
    fs::read_to_string(&f.log).unwrap_or_default()
}

#[test]
fn merge_with_yes_merges_updates_main_and_deletes_the_branch() {
    let f = fx();
    let o = repo(&f, &["pr", "merge", "8", "--yes"], "", &[]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        gh_log(&f).contains("pr merge 8 --merge --delete-branch"),
        "{}",
        gh_log(&f)
    );
    assert_eq!(git(&f.work, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(git(&f.work, &["log", "-1", "--format=%s"]), "merge feature");
    assert!(git(&f.work, &["branch", "--list", "feature"]).is_empty());
}

#[test]
fn merge_asks_and_an_empty_answer_cancels_with_success() {
    let f = fx();
    let o = repo(&f, &["pr", "merge", "8"], "", &[]);
    assert!(o.status.success());
    assert!(
        text(&o.stdout).contains("Merge this pull request? [y/N] "),
        "{}",
        text(&o.stdout)
    );
    assert!(text(&o.stdout).ends_with("Cancelled.\n"));
    assert!(!gh_log(&f).contains("pr merge"));
}

#[test]
fn merge_answering_y_merges() {
    let f = fx();
    let o = repo(&f, &["pr", "merge", "8"], "y\n", &[]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(gh_log(&f).contains("pr merge 8"));
}

#[test]
fn a_conflicting_pr_fails_with_exit_1_and_is_not_merged() {
    let f = fx();
    let o = repo(
        &f,
        &["pr", "merge", "8", "-y"],
        "",
        &[("GH_MERGEABLE", "CONFLICTING")],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("This pull request is not ready to merge."),
        "{}",
        text(&o.stderr)
    );
    assert!(!gh_log(&f).contains("pr merge"));
}

#[test]
fn an_unknown_mergeability_is_retried_then_refused() {
    let f = fx();
    let o = repo(
        &f,
        &["pr", "merge", "8", "-y"],
        "",
        &[
            ("GH_MERGEABLE", "UNKNOWN"),
            ("DOTFILES_PR_MERGEABLE_TRIES", "2"),
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    let err = text(&o.stderr);
    assert!(err.contains("(1/2).") && err.contains("(2/2)."), "{err}");
}

#[test]
fn a_bad_argument_is_a_usage_error_with_exit_2() {
    let f = fx();
    for args in [["pr", "merge", "abc"], ["pr", "merge", "--nope"]] {
        let o = repo(&f, &args, "", &[]);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", text(&o.stderr));
    }
    assert!(!f.log.exists());
}

#[test]
fn an_invalid_wait_variable_fails_loudly() {
    let f = fx();
    let o = repo(
        &f,
        &["pr", "merge", "8", "-y"],
        "",
        &[("DOTFILES_PR_MERGEABLE_TRIES", "many")],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("DOTFILES_PR_MERGEABLE_TRIES"),
        "{}",
        text(&o.stderr)
    );
}

#[test]
fn create_with_flags_opens_the_pr_against_the_base() {
    let f = fx();
    let o = repo(
        &f,
        &[
            "pr", "create", "--base", "main", "--title", "T", "--body", "B",
        ],
        "",
        &[],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        gh_log(&f).contains("pr create --base main --head feature --title T --body B"),
        "{}",
        gh_log(&f)
    );
    assert!(text(&o.stdout).contains("https://github.com/o/r/pull/9"));
}

#[test]
fn create_reads_the_title_and_description_from_stdin() {
    let f = fx();
    let o = repo(
        &f,
        &["pr", "create", "--base", "main"],
        "My title\nline 1\nline 2\n",
        &[],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        gh_log(&f).contains("--title My title --body line 1\nline 2\n"),
        "{}",
        gh_log(&f)
    );
}

#[test]
fn create_from_main_is_refused() {
    let f = fx();
    git(&f.work, &["checkout", "-q", "main"]);
    let o = repo(
        &f,
        &[
            "pr", "create", "--base", "main", "--title", "T", "--body", "B",
        ],
        "",
        &[],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("You are already on main. Create a feature branch first."),
        "{}",
        text(&o.stderr)
    );
    assert!(!gh_log(&f).contains("pr create"));
}

#[test]
fn create_without_changes_is_refused() {
    let f = fx();
    git(&f.work, &["checkout", "-q", "-b", "same", "main"]);
    let o = repo(
        &f,
        &[
            "pr", "create", "--base", "main", "--title", "T", "--body", "B",
        ],
        "",
        &[],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("No changes detected between main and same."),
        "{}",
        text(&o.stderr)
    );
}

#[test]
fn a_missing_gh_fails_loudly_and_merges_nothing() {
    let f = fx();
    let only_git = f.bin.join("nogh");
    ok(fs::create_dir_all(&only_git));
    let git_path = text(&ok(Command::new("sh").args(["-c", "command -v git"]).output()).stdout);
    ok(symlink(git_path.trim(), only_git.join("git")));
    let o = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(["pr", "merge", "8", "-y"])
        .current_dir(&f.work)
        .env("PATH", &only_git)
        .output());
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("gh not found"),
        "{}",
        text(&o.stderr)
    );
}
