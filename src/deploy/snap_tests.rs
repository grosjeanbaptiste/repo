use super::*;
use crate::deploy::fake::{Fake, drive, message};
use crate::deploy::target::Env;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn project(snap: bool) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    if snap {
        ok(fs::create_dir_all(d.path().join("build/snap")));
        ok(fs::write(d.path().join("build/snap/app.snap"), "x"));
    }
    d
}

fn go(root: &std::path::Path, env: Env, creds_env: &[(&str, &str)], tool: Fake) -> (Result<()>, Fake) {
    drive(env, root, creds_env, tool, upload)
}

fn with_secret(secret: Option<&str>) -> Fake {
    Fake { captures: vec![("dotfiles".into(), "secret".into(), secret.map(String::from))], ..Fake::default() }
}

const CRED: [(&str, &str); 1] = [("SNAPCRAFT_STORE_CREDENTIALS", "tok")];

#[test]
fn prod_uploads_to_stable_with_the_credentials_in_the_environment() {
    let d = project(true);
    let (r, t) = go(d.path(), Env::Prod, &CRED, Fake::default());
    assert!(r.is_ok(), "{}", message(&r));
    let snap = d.path().join("build/snap/app.snap");
    let runs = t.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!((runs[0].prog.as_str(), runs[0].args.clone()), ("snapcraft", vec!["upload".into(), snap.display().to_string(), "--release=stable".into()]));
    assert_eq!(runs[0].env, [("SNAPCRAFT_STORE_CREDENTIALS".to_string(), "tok".to_string())]);
}

#[test]
fn test_uploads_to_beta() {
    let d = project(true);
    let (_, t) = go(d.path(), Env::Test, &CRED, Fake::default());
    assert!(t.lines()[0].ends_with("--release=beta"));
}

#[test]
fn credentials_come_from_the_secret_store_when_the_environment_has_none() {
    let d = project(true);
    let (r, t) = go(d.path(), Env::Prod, &[], with_secret(Some("from-secret")));
    assert!(r.is_ok(), "{}", message(&r));
    assert_eq!(t.runs()[0].env, [("SNAPCRAFT_STORE_CREDENTIALS".to_string(), "from-secret".to_string())]);
}

#[test]
fn no_credentials_anywhere_gives_the_enrolment_command() {
    let d = project(true);
    for fake in [with_secret(None), with_secret(Some(""))] {
        let (r, t) = go(d.path(), Env::Prod, &[], fake);
        assert_eq!(message(&r), "no Snap Store credentials — enroll with:\n  snapcraft export-login - | dotfiles secret add snapcraft-store-credentials");
        assert!(t.runs().is_empty());
    }
}

#[test]
fn a_missing_snapcraft_is_refused_first() {
    let d = project(true);
    let (r, _) = go(d.path(), Env::Prod, &CRED, Fake { missing: vec!["snapcraft".into()], ..Fake::default() });
    assert_eq!(message(&r), "snapcraft not found — Snap publish needs Linux + `snapcraft login`.");
}

#[test]
fn a_missing_snap_asks_for_a_pack() {
    let d = project(false);
    let (r, t) = go(d.path(), Env::Prod, &CRED, Fake::default());
    assert_eq!(message(&r), format!("no .snap under {}/build (run `snapcraft pack` first).", d.path().display()));
    assert!(t.runs().is_empty());
}

#[test]
fn a_failing_upload_is_an_error() {
    let d = project(true);
    assert!(go(d.path(), Env::Prod, &CRED, Fake { failing: vec!["snapcraft".into()], ..Fake::default() }).0.is_err());
}
