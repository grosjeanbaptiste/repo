use super::*;
use crate::deploy::fake::{Fake, drive, message};
use crate::deploy::target::Env;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn project(msix: bool) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    if msix {
        ok(fs::create_dir_all(d.path().join("build/windows")));
        ok(fs::write(d.path().join("build/windows/app.msix"), "x"));
    }
    d
}

fn go(root: &std::path::Path, env: Env, tool: Fake) -> (Result<()>, Fake) {
    drive(env, root, &[], tool, upload)
}

#[test]
fn prod_publishes_to_production_and_test_to_flight() {
    let d = project(true);
    let msix = d.path().join("build/windows/app.msix");
    for (env, channel) in [(Env::Prod, "production"), (Env::Test, "flight")] {
        let (r, t) = go(d.path(), env, Fake::default());
        assert!(r.is_ok(), "{}", message(&r));
        assert_eq!(t.lines(), vec![format!("msstore publish {} --channel {channel}", msix.display())]);
    }
}

#[test]
fn a_missing_msstore_is_refused_first() {
    let d = project(true);
    let (r, t) = go(d.path(), Env::Prod, Fake { missing: vec!["msstore".into()], ..Fake::default() });
    assert_eq!(message(&r), "msstore not found — MS Store publish needs Windows + Partner Center creds.");
    assert!(t.runs().is_empty());
}

#[test]
fn a_missing_msix_says_how_to_package() {
    let d = project(false);
    let (r, _) = go(d.path(), Env::Prod, Fake::default());
    assert_eq!(message(&r), format!("no .msix under {}/build (package with `dart run msix:create`).", d.path().display()));
}

#[test]
fn a_failing_publish_is_an_error() {
    let d = project(true);
    assert!(go(d.path(), Env::Prod, Fake { failing: vec!["msstore".into()], ..Fake::default() }).0.is_err());
}
