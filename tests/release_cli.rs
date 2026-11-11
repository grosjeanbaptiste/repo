//! `repo release` end to end: a real bash sources a profile and an `.envrc`, then runs a scripted release script.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn script(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        ok(fs::create_dir_all(dir));
    }
    ok(fs::write(path, text));
    ok(fs::set_permissions(path, fs::Permissions::from_mode(0o755)));
}

const PRINTS: &str = "#!/bin/sh\necho \"profile=$FROM_PROFILE envrc=$FROM_ENVRC marker=$MARKER\"\necho \"cwd=$(pwd)\"\nfor a in \"$@\"; do echo \"arg=$a\"; done\n";

struct Fx {
    _d: tempfile::TempDir,
    base: PathBuf,
    project: PathBuf,
}

fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let project = base.join("app");
    for p in [
        &project,
        &base.join("state/shell"),
        &base.join("bin"),
        &base.join("cwd"),
    ] {
        ok(fs::create_dir_all(p));
    }
    ok(fs::write(
        base.join("state/shell/profile.sh"),
        "FROM_PROFILE=yes\n",
    ));
    ok(fs::write(
        project.join(".envrc"),
        "FROM_ENVRC=yes\nMARKER=$(cat ./marker.txt)\n",
    ));
    ok(fs::write(project.join("marker.txt"), "relative-ok\n"));
    script(&project.join("stores/scripts/tf_release.py"), PRINTS);
    script(
        &base.join("bin/python3"),
        "#!/bin/sh\nshift\nexec /bin/sh \"$0.real\" \"$@\"\n",
    );
    ok(fs::write(base.join("bin/python3.real"), PRINTS));
    Fx {
        _d: d,
        base,
        project,
    }
}

impl Fx {
    fn release(&self, args: &[&str]) -> Output {
        let path = format!("{}:/usr/bin:/bin", self.base.join("bin").display());
        ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("release")
            .args(args)
            .current_dir(self.base.join("cwd"))
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.base.join("state"))
            .env("DOTFILES_STATE_ROOT", self.base.join("state"))
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
fn the_script_sees_profile_and_envrc_runs_where_we_were_invoked_and_gets_the_arguments() {
    let f = fx();
    let o = f.release(&[
        "--path",
        &f.project.to_string_lossy(),
        "--skip-upload",
        "x y",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let t = out(&o);
    assert!(
        t.contains("profile=yes envrc=yes marker=relative-ok"),
        "{t}"
    );
    assert!(
        t.contains(&format!("cwd={}", f.base.join("cwd").display())),
        "{t}"
    );
    assert!(t.contains("arg=--skip-upload\narg=x y\n"), "{t}");
    assert!(err(&o).is_empty(), "{}", err(&o));
}

#[test]
fn a_script_option_replaces_the_convention() {
    let f = fx();
    script(&f.project.join("ci/ship.sh"), "echo shipped \"$@\"\n");
    let o = f.release(&[
        "--path",
        &f.project.to_string_lossy(),
        "--script",
        "ci/ship.sh",
        "--",
        "--path",
        "p",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(out(&o), "shipped --path p\n");
}

#[test]
fn no_script_exits_1_with_where_it_looked() {
    let f = fx();
    let bare = f.base.join("bare");
    ok(fs::create_dir(&bare));
    let o = f.release(&["--path", &bare.to_string_lossy()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("no release script found in ")
            && err(&o).contains("Tried: stores/scripts/tf_release.py"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_failing_profile_is_said_and_the_script_still_runs() {
    let f = fx();
    ok(fs::write(
        f.base.join("state/shell/profile.sh"),
        "echo boom >&2\nfalse\n",
    ));
    let o = f.release(&["--path", &f.project.to_string_lossy()]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(
        err(&o).contains("warn: release: ")
            && err(&o).contains("exited with status 1")
            && err(&o).contains("boom"),
        "{}",
        err(&o)
    );
    assert!(out(&o).contains("envrc=yes"), "{}", out(&o));
}

#[test]
fn a_quiet_profile_prints_nothing() {
    let f = fx();
    ok(fs::write(
        f.base.join("state/shell/profile.sh"),
        "echo noise >&2\nFROM_PROFILE=yes\n",
    ));
    let o = f.release(&["--path", &f.project.to_string_lossy()]);
    assert!(!err(&o).contains("noise"), "{}", err(&o));
}

#[test]
fn require_audit_aborts_before_the_script_when_a_variable_is_missing() {
    let f = fx();
    ok(fs::write(
        f.project.join(".env.example"),
        "NOT_SET_ANYWHERE=\n",
    ));
    let o = f.release(&["--require-audit", "--path", &f.project.to_string_lossy()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("release: audit failed for ") && err(&o).contains("aborting"),
        "{}",
        err(&o)
    );
    assert!(!out(&o).contains("profile="), "{}", out(&o));
}

#[test]
fn require_audit_runs_the_script_when_the_audit_passes() {
    let f = fx();
    let o = f.release(&["--require-audit", "--path", &f.project.to_string_lossy()]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let t = out(&o);
    let (audit, run) = (t.find("== auditing"), t.find("profile="));
    assert!(audit.zip(run).is_some_and(|(a, r)| a < r), "{t}");
}

#[test]
fn help_prints_the_usage_and_exits_0() {
    let o = fx().release(&["--help"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(
        out(&o).starts_with("Usage:\n  repo release "),
        "{}",
        out(&o)
    );
}

#[test]
fn a_path_that_is_not_a_directory_and_a_flag_without_value_exit_1() {
    let f = fx();
    let o = f.release(&["--path", "/no/such/dir"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("release: path is not a directory"),
        "{}",
        err(&o)
    );
    let o = f.release(&["--script"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("--script needs a value"), "{}", err(&o));
}

#[test]
fn a_python_script_runs_under_python3() {
    let f = fx();
    let o = f.release(&["--path", &f.project.to_string_lossy(), "beta"]);
    assert!(out(&o).contains("arg=beta"), "{}", out(&o));
}
