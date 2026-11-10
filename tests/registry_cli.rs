//! `repo list|scan|status` end to end on a temp state root and temp repositories. No network.
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn repo(state: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(args)
        .env("DOTFILES_STATE_ROOT", state)
        .env("HOME", home)
        .output()
        .unwrap_or_else(|e| panic!("cannot run repo: {e}"))
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn mkrepo(root: &Path, name: &str) {
    let p = root.join(name);
    ok(fs::create_dir_all(&p));
    let s = Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(&p)
        .status();
    assert!(ok(s).success());
}

struct Fx {
    _d: tempfile::TempDir,
    state: std::path::PathBuf,
    home: std::path::PathBuf,
}

fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let (state, home) = (d.path().join("state"), d.path().join("home"));
    ok(fs::create_dir_all(&state));
    ok(fs::create_dir_all(&home));
    Fx { _d: d, state, home }
}

#[test]
fn list_with_no_registry_says_how_to_create_it_and_succeeds() {
    let f = fx();
    let o = repo(&f.state, &f.home, &["list"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(out(&o), "");
    assert!(
        err(&o).contains("No projects registered yet. Run: repo scan"),
        "{}",
        err(&o)
    );
}

#[test]
fn scan_then_list_then_status_round_trip() {
    let f = fx();
    mkrepo(&f.home, "alpha");
    mkrepo(&f.home, "beta");
    let o = repo(&f.state, &f.home, &["scan"]); // default root is $HOME
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).starts_with("✓ Scan complete: 2 added, 0 unchanged, 0 removed"),
        "{}",
        out(&o)
    );
    let o = repo(&f.state, &f.home, &["list"]);
    assert!(
        out(&o).contains("alpha") && out(&o).contains("beta"),
        "{}",
        out(&o)
    );
    let o = repo(&f.state, &f.home, &["status", "alpha"]);
    assert!(out(&o).starts_with("Project: alpha\n"), "{}", out(&o));
    assert!(out(&o).contains("Branch:  main"), "{}", out(&o));
    let o = repo(&f.state, &f.home, &["status"]);
    assert!(out(&o).starts_with("NAME"), "{}", out(&o));
    assert_eq!(out(&o).lines().count(), 3);
}

#[test]
fn scan_takes_explicit_roots_and_is_idempotent() {
    let f = fx();
    let extra = f.home.join("extra");
    mkrepo(&extra, "one");
    let root = extra.display().to_string();
    assert!(out(&repo(&f.state, &f.home, &["scan", &root])).contains("1 added"));
    assert!(out(&repo(&f.state, &f.home, &["scan", &root])).contains("0 added, 1 unchanged"));
}

#[test]
fn status_of_an_unknown_project_fails_loudly() {
    let f = fx();
    let o = repo(&f.state, &f.home, &["status", "nope"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("Unknown project: nope"), "{}", err(&o));
}

#[test]
fn status_of_a_registered_non_repository_fails_with_the_report() {
    let f = fx();
    let plain = f.home.join("plain");
    ok(fs::create_dir(&plain));
    ok(fs::write(
        f.state.join("projects.txt"),
        format!("plain|{}\n", plain.display()),
    ));
    let o = repo(&f.state, &f.home, &["status", "plain"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("Not a git repository (or path missing)."),
        "{}",
        err(&o)
    );
}

#[test]
fn a_malformed_registry_fails_every_command_loudly() {
    let f = fx();
    ok(fs::write(f.state.join("projects.txt"), "garbage\n"));
    for args in [&["list"][..], &["status"], &["status", "x"], &["scan"]] {
        let o = repo(&f.state, &f.home, args);
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        assert!(err(&o).contains("name|path"), "{args:?}: {}", err(&o));
    }
}

#[test]
fn scan_reports_unreadable_directories_on_stderr_and_still_succeeds() {
    use std::os::unix::fs::PermissionsExt;
    let f = fx();
    mkrepo(&f.home, "alpha");
    let locked = f.home.join("locked");
    ok(fs::create_dir(&locked));
    ok(fs::set_permissions(
        &locked,
        fs::Permissions::from_mode(0o0),
    ));
    let o = repo(&f.state, &f.home, &["scan"]);
    ok(fs::set_permissions(
        &locked,
        fs::Permissions::from_mode(0o755),
    ));
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        err(&o).contains("warn:") && err(&o).contains("locked"),
        "{}",
        err(&o)
    );
}

#[test]
fn no_state_root_and_no_home_fails_by_name() {
    let o = Command::new(env!("CARGO_BIN_EXE_repo"))
        .arg("list")
        .env_remove("DOTFILES_STATE_ROOT")
        .env_remove("HOME")
        .output()
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("HOME is not set"), "{}", err(&o));
}

#[test]
fn status_of_a_repo_without_origin_or_upstream_prints_nothing_on_stderr() {
    let f = fx();
    mkrepo(&f.home, "alpha");
    repo(&f.state, &f.home, &["scan"]);
    let o = repo(&f.state, &f.home, &["status", "alpha"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(err(&o), "");
    assert!(out(&o).contains("Remote:  none"));
}
