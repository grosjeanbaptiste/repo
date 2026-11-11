use super::*;
use crate::run::fake::Fake;
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        ok(fs::create_dir_all(parent));
    }
    ok(fs::write(p, text));
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn go(dir: &Path, verb: &str, args: &[&str], runner: &Fake) -> (Result<Outcome>, String) {
    let mut err = vec![];
    let r = dispatch(dir, verb, &s(args), runner, &mut err);
    (r, String::from_utf8_lossy(&err).into_owned())
}

fn calls(r: &Fake) -> Vec<(String, Vec<String>)> {
    r.calls.borrow().iter().map(|(p, a, _)| (p.clone(), a.clone())).collect()
}

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    for (rel, text) in files {
        put(d.path(), rel, text);
    }
    d
}

fn dir_arg(d: &Path) -> String {
    d.display().to_string()
}

#[test]
fn a_makefile_target_runs_make_in_the_directory() {
    let d = project(&[("Makefile", "build:\n")]);
    let r = Fake::exiting(0);
    let (o, e) = go(d.path(), "build", &[], &r);
    assert_eq!(ok(o), Outcome::Done);
    assert!(e.is_empty());
    assert_eq!(calls(&r), vec![("make".into(), s(&["-C", &dir_arg(d.path()), "build"]))]);
    assert_eq!(r.calls.borrow()[0].2, None);
}

#[test]
fn makefile_extra_arguments_become_one_args_variable() {
    let d = project(&[("Makefile", "go:\n")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "go", &["--action=publish", "--channel=test"], &r).0);
    assert_eq!(calls(&r)[0].1, s(&["-C", &dir_arg(d.path()), "go", "ARGS=--action=publish --channel=test"]));
}

#[test]
fn a_failing_command_propagates_its_own_exit_code() {
    let d = project(&[("Makefile", "build:\n")]);
    let (o, _) = go(d.path(), "build", &[], &Fake::exiting(66));
    assert_eq!(ok(o), Outcome::Failed(66));
}

#[test]
fn an_unknown_verb_starts_nothing() {
    for files in [vec![("Makefile", "a:\n")], vec![("Cargo.toml", "")], vec![("pubspec.yaml", "")], vec![]] {
        let d = project(&files);
        let r = Fake::exiting(0);
        let (o, e) = go(d.path(), "nope", &[], &r);
        assert_eq!(ok(o), Outcome::Unknown);
        assert!(e.is_empty() && calls(&r).is_empty());
    }
}

#[test]
fn a_makefile_target_wins_over_a_melos_script_of_the_same_name() {
    let d = project(&[("Makefile", "gen:\n"), ("melos.yaml", "scripts:\n  gen:\n")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "gen", &[], &r).0);
    assert_eq!(calls(&r)[0].0, "make");
}

#[test]
fn a_melos_script_runs_through_melos_in_the_directory() {
    let d = project(&[("Makefile", "a:\n"), ("melos.yaml", "scripts:\n  gen:\n")]);
    let r = Fake::exiting(3);
    let (o, e) = go(d.path(), "gen", &[], &r);
    assert_eq!(ok(o), Outcome::Failed(3));
    assert!(e.is_empty());
    assert_eq!(calls(&r), vec![("melos".into(), s(&["run", "gen"]))]);
    assert_eq!(r.calls.borrow()[0].2.as_deref(), Some(d.path()));
}

#[test]
fn melos_scripts_run_on_any_kind_and_extra_arguments_are_reported_as_ignored() {
    let d = project(&[("Cargo.toml", ""), ("melos.yaml", "scripts:\n  gen:\n")]);
    let r = Fake::exiting(0);
    let (o, e) = go(d.path(), "gen", &["--x"], &r);
    assert_eq!(ok(o), Outcome::Done);
    assert!(e.contains("warn:") && e.contains("--x") && e.contains("melos"), "{e}");
    assert_eq!(calls(&r), vec![("melos".into(), s(&["run", "gen"]))]);
}

#[test]
fn a_cargo_verb_wins_over_a_melos_script_and_forwards_the_arguments() {
    let d = project(&[("Cargo.toml", ""), ("melos.yaml", "scripts:\n  test:\n")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "test", &["--", "-q"], &r).0);
    assert_eq!(calls(&r), vec![("cargo".into(), s(&["test", "--", "-q"]))]);
    assert_eq!(r.calls.borrow()[0].2.as_deref(), Some(d.path()));
}

#[test]
fn every_cargo_verb_is_forwarded_as_is() {
    let d = project(&[("Cargo.toml", "")]);
    for v in ["run", "build", "test", "check", "clippy"] {
        let r = Fake::exiting(0);
        ok(go(d.path(), v, &[], &r).0);
        assert_eq!(calls(&r), vec![("cargo".into(), s(&[v]))]);
    }
}

#[test]
fn flutter_verbs_run_flutter_in_the_directory_and_report_ignored_arguments() {
    let d = project(&[("pubspec.yaml", "")]);
    for (verb, want) in [("test", s(&["test"])), ("build", s(&["build"])), ("clean", s(&["clean"])), ("pub-get", s(&["pub", "get"]))] {
        let r = Fake::exiting(0);
        let (o, e) = go(d.path(), verb, &["--x"], &r);
        assert_eq!(ok(o), Outcome::Done);
        assert!(e.contains("warn:") && e.contains("--x"), "{e}");
        assert_eq!(calls(&r), vec![("flutter".into(), want)]);
        assert_eq!(r.calls.borrow()[0].2.as_deref(), Some(d.path()));
    }
}

#[test]
fn flutter_run_goes_to_dotfiles_which_picks_the_device() {
    let d = project(&[("pubspec.yaml", "")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "run", &[], &r).0);
    assert_eq!(calls(&r), vec![("dotfiles".into(), s(&["project", &dir_arg(d.path()), "run"]))]);
}

#[test]
fn publish_needs_an_environment_and_a_store() {
    let d = project(&[("Makefile", "publish:\n")]);
    for args in [vec![], vec!["prod"]] {
        let r = Fake::exiting(0);
        let (o, e) = go(d.path(), "publish", &args, &r);
        assert_eq!(ok(o), Outcome::Failed(2));
        assert!(e.contains("Usage") && e.contains("publish <prod|test> <store>"), "{e}");
        assert!(calls(&r).is_empty());
    }
}

#[test]
fn publish_runs_repo_deploy_on_the_directory_with_every_option() {
    let d = project(&[("Makefile", "publish:\n")]);
    let r = Fake::exiting(5);
    let (o, _) = go(d.path(), "publish", &["test", "ios", "--dry-run"], &r);
    assert_eq!(ok(o), Outcome::Failed(5));
    assert_eq!(
        calls(&r),
        vec![("repo-bin".into(), s(&["deploy", "test", "ios", &dir_arg(d.path()), "--dry-run"]))]
    );
}

#[test]
fn a_store_release_script_routes_publish_to_repo_release_for_testflight_only() {
    let d = project(&[("stores/scripts/tf_release.py", "x")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "publish", &["test", "ios", "--x"], &r).0);
    assert_eq!(
        calls(&r),
        vec![("repo-bin".into(), s(&["release", "--path", &dir_arg(d.path()), "--x"]))]
    );
    for args in [["prod", "ios"], ["test", "android"]] {
        let r = Fake::exiting(0);
        let (o, e) = go(d.path(), "publish", &args, &r);
        assert_eq!(ok(o), Outcome::Failed(2));
        assert_eq!(e, "This Dioxus release script supports TestFlight only: use publish test ios.\n");
        assert!(calls(&r).is_empty());
    }
}

#[test]
fn install_with_a_pubspec_takes_exactly_one_device_and_is_delegated() {
    let d = project(&[("pubspec.yaml", "")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "install", &["iphone"], &r).0);
    assert_eq!(calls(&r), vec![("dotfiles".into(), s(&["project", &dir_arg(d.path()), "install", "iphone"]))]);
    for args in [vec![], vec!["a", "b"]] {
        let r = Fake::exiting(0);
        let (o, e) = go(d.path(), "install", &args, &r);
        assert_eq!(ok(o), Outcome::Failed(2));
        assert!(e.contains("install <device>"), "{e}");
        assert!(calls(&r).is_empty());
    }
}

#[test]
fn install_with_a_device_hook_requires_it_to_be_executable() {
    let d = project(&[("scripts/device-install.sh", "#!/bin/sh\n")]);
    let r = Fake::exiting(0);
    let e = go(d.path(), "install", &["iphone"], &r).0.err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("not executable")));
    assert!(calls(&r).is_empty());
    ok(fs::set_permissions(d.path().join("scripts/device-install.sh"), fs::Permissions::from_mode(0o755)));
    ok(go(d.path(), "install", &["iphone"], &r).0);
    assert_eq!(calls(&r)[0].1[2], "install");
    let (o, _) = go(d.path(), "install", &[], &r);
    assert_eq!(ok(o), Outcome::Failed(2));
}

#[test]
fn install_without_hook_or_pubspec_is_an_ordinary_makefile_target() {
    let d = project(&[("Makefile", "install:\n")]);
    let r = Fake::exiting(0);
    ok(go(d.path(), "install", &[], &r).0);
    assert_eq!(calls(&r)[0].0, "make");
}

#[test]
fn a_program_that_cannot_start_is_an_error_not_an_exit_code() {
    let d = project(&[("Cargo.toml", "")]);
    let r = Fake { calls: RefCell::new(vec![]), code: 0, spawn_error: true };
    assert!(go(d.path(), "build", &[], &r).0.is_err());
}

#[test]
fn an_unreadable_makefile_is_an_error() {
    let d = project(&[("Makefile", "a:\n")]);
    ok(fs::set_permissions(d.path().join("Makefile"), fs::Permissions::from_mode(0o000)));
    assert!(go(d.path(), "a", &[], &Fake::exiting(0)).0.is_err());
}
