use super::diag::fake::Fake;
use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn go(root: &Path, status: &dyn Fn(&str) -> Status, fake: &Fake) -> (anyhow::Result<()>, String, String) {
    let has = fake.has();
    let host = fake.host(&has);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = run(root, status, &host, &mut out, &mut err);
    (r, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

fn why(r: anyhow::Result<()>) -> String {
    r.err().map(|e| e.to_string()).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn an_empty_project_audits_clean() {
    let d = ok(tempfile::tempdir());
    let (r, out, err) = go(d.path(), &|_| Status::Missing, &Fake::new(&[]));
    assert!(r.is_ok(), "{r:?}");
    assert!(out.starts_with(&format!("== auditing {} ==\n", d.path().display())), "{out}");
    assert!(err.is_empty());
}

#[test]
fn a_missing_variable_prints_the_report_then_fails() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join(".env.example"), "SECRET=1\nFINE=2\n"));
    let status = |v: &str| if v == "FINE" { Status::InEnv } else { Status::Missing };
    let (r, out, _) = go(d.path(), &status, &Fake::new(&[]));
    assert!(out.contains("1 in env, 0 in dotfiles (not exported), 1 missing"), "{out}");
    assert_eq!(why(r), "audit: 1 variable(s) missing");
}

#[test]
fn a_failed_check_fails_the_audit() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join(".gitmodules"), ""));
    let fake = Fake::new(&[]).answering("git -C", Err("broken"));
    let (r, out, _) = go(d.path(), &|_| Status::InEnv, &fake);
    assert!(out.contains("0 ok, 0 warn, 1 fail"), "{out}");
    assert_eq!(why(r), "audit: 1 check(s) failed");
}

#[test]
fn both_problems_are_named() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join(".gitmodules"), ""));
    ok(fs::write(d.path().join(".env.example"), "X=1\n"));
    let fake = Fake::new(&[]).answering("git -C", Err("broken"));
    assert_eq!(why(go(d.path(), &|_| Status::Missing, &fake).0), "audit: 1 variable(s) missing, 1 check(s) failed");
}

#[test]
fn warnings_alone_do_not_fail() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("mise.toml"), ""));
    let (r, out, _) = go(d.path(), &|_| Status::InEnv, &Fake::new(&[]));
    assert!(r.is_ok(), "{r:?}");
    assert!(out.contains("1 warn"), "{out}");
}

#[test]
fn an_unreadable_directory_is_a_warn_line_on_stderr() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("locked")));
    let locked = d.path().join("locked");
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)));
    let (r, _, err) = go(d.path(), &|_| Status::InEnv, &Fake::new(&[]));
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)));
    assert!(r.is_ok(), "{r:?}");
    assert!(err.starts_with("warn: cannot read ") && err.contains("locked"), "{err}");
}

#[test]
fn an_unreadable_root_is_an_error() {
    let d = ok(tempfile::tempdir());
    assert!(go(&d.path().join("nope"), &|_| Status::InEnv, &Fake::new(&[])).0.is_err());
}
