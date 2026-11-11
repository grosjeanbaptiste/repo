//! `repo doctor` end to end: a real git repository, a scripted `gh`, a controlled environment.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) {
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
}

const GH: &str = r#"#!/bin/sh
case "$*" in
  "--version") echo "gh version 2" ;;
  "pr list"*) printf '1\037me\0372026-01-01T00:00:00Z\n2\037bob\0372099-01-01T00:00:00Z\n' ;;
  "api user"*) echo me ;;
  "repo view"*) echo "me/ss trunk" ;;
  "run list"*) printf 'CI\037success\037completed\0372026-11-09\n' ;;
  *) echo "unexpected: $*" >&2; exit 3 ;;
esac
"#;

struct Fx {
    _d: tempfile::TempDir,
    work: PathBuf,
    bin: PathBuf,
}

fn fx(remote: &str) -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let (work, bin) = (base.join("work"), base.join("bin"));
    for p in [&work, &bin, &base.join("state/secrets")] {
        ok(fs::create_dir_all(p));
    }
    git(&work, &["init", "-q", "-b", "main"]);
    ok(fs::write(work.join("a.rs"), "// TODO later\n"));
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "first"]);
    if !remote.is_empty() {
        git(&work, &["remote", "add", "origin", remote]);
    }
    Fx { _d: d, work, bin }
}

impl Fx {
    fn with_gh(&self) {
        let gh = self.bin.join("gh");
        ok(fs::write(&gh, GH));
        ok(fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)));
    }

    fn doctor(&self, arg: &Path) -> Output {
        let state = self
            .bin
            .parent()
            .map(|p| p.join("state"))
            .unwrap_or_default();
        let path = format!("{}:/usr/bin:/bin", self.bin.display());
        ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("doctor")
            .arg(arg)
            .env_clear()
            .env("PATH", path)
            .env("HOME", &state)
            .env("DOTFILES_STATE_ROOT", &state)
            .output())
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn the_report_covers_git_github_quality_license_and_audit() {
    let f = fx("git@github.com:me/ss.git");
    f.with_gh();
    let o = f.doctor(&f.work);
    let t = out(&o);
    assert_eq!(o.status.code(), Some(0), "{t}\n{}", err(&o));
    assert!(
        t.starts_with(&format!("== project doctor: {} ==\n", f.work.display())),
        "{t}"
    );
    for want in [
        "Branch:                main",
        "(no upstream)",
        "Working tree:          0 uncommitted change(s)",
        "Open PRs:              2 total, 1 mine",
        "Stale:                 1 PR(s) inactive >30 days",
        "CI on trunk:           CI: success (2026-11-09)",
        "Files with TODO:       1",
        "Touched (last 7d):     1 file(s)",
        "License:               NONE",
        "  == Audit ==\n    == auditing",
    ] {
        assert!(t.contains(want), "missing {want:?} in\n{t}");
    }
    assert!(err(&o).is_empty(), "{}", err(&o));
}

#[test]
fn without_the_platform_tool_the_report_says_so_and_goes_on() {
    let f = fx("git@github.com:me/ss.git");
    let o = f.doctor(&f.work);
    let t = out(&o);
    assert_eq!(o.status.code(), Some(0), "{t}\n{}", err(&o));
    assert!(
        t.contains("(platform tool unavailable: gh not found"),
        "{t}"
    );
    assert!(t.contains("== Audit =="), "{t}");
}

#[test]
fn a_repository_without_origin_says_so() {
    let f = fx("");
    f.with_gh();
    let t = out(&f.doctor(&f.work));
    assert!(t.contains("(no origin remote configured)"), "{t}");
}

#[test]
fn a_missing_variable_fails_with_exit_1_after_the_whole_report() {
    let f = fx("");
    ok(fs::write(f.work.join(".env.example"), "MISSING_VAR=\n"));
    let o = f.doctor(&f.work);
    assert_eq!(o.status.code(), Some(1));
    assert!(out(&o).contains("1 missing"), "{}", out(&o));
    assert!(
        err(&o).contains("doctor: audit failed (audit: 1 variable(s) missing)"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_path_that_is_not_a_directory_fails_before_any_report() {
    let f = fx("");
    let o = f.doctor(&f.work.join("a.rs"));
    assert_eq!(o.status.code(), Some(1));
    assert!(out(&o).is_empty());
    assert!(
        err(&o).contains("doctor: path is not a directory"),
        "{}",
        err(&o)
    );
}
