use super::*;
use crate::deploy::fake::{Fake, drive, message};
use crate::deploy::target::Env;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

struct Fx {
    root: tempfile::TempDir,
    sa: String,
}

fn fx(aab: bool, sa: bool) -> Fx {
    let root = ok(tempfile::tempdir());
    if aab {
        let d = root.path().join("build/app/outputs/bundle/release");
        ok(fs::create_dir_all(&d));
        ok(fs::write(d.join("app-release.aab"), "x"));
    }
    let sa_path = root.path().join("sa.json");
    if sa {
        ok(fs::write(&sa_path, "{}"));
    }
    Fx { sa: sa_path.display().to_string(), root }
}

fn go(f: &Fx, env: Env, extra: &[(&str, &str)], tool: Fake) -> (Result<()>, Fake) {
    let pairs = [&[("PLAY_PACKAGE_NAME", "com.x"), ("PLAY_SERVICE_ACCOUNT_PATH", f.sa.as_str()), ("STORES_PYTHON", "/py"), ("STORES_SCRIPTS_DIR", "/s")][..], extra].concat();
    drive(env, f.root.path(), &pairs, tool, upload)
}

#[test]
fn prod_uploads_to_production_with_the_package_and_account_in_the_environment() {
    let f = fx(true, true);
    let (r, t) = go(&f, Env::Prod, &[], Fake::default());
    assert!(r.is_ok(), "{}", message(&r));
    let aab = f.root.path().join("build/app/outputs/bundle/release/app-release.aab");
    let runs = t.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].prog, "/py");
    assert_eq!(runs[0].args, ["/s/play_upload.py", "--aab", aab.to_str().unwrap_or_default(), "--track", "production"]);
    assert_eq!(runs[0].env, [("PLAY_PACKAGE_NAME".to_string(), "com.x".to_string()), ("PLAY_SERVICE_ACCOUNT_PATH".to_string(), f.sa.clone())]);
}

#[test]
fn test_uploads_to_the_internal_track() {
    let f = fx(true, true);
    let (_, t) = go(&f, Env::Test, &[], Fake::default());
    assert_eq!(t.runs()[0].args.last().map(String::as_str), Some("internal"));
}

#[test]
fn without_a_bundle_the_build_is_asked_for() {
    let f = fx(false, true);
    let (r, t) = go(&f, Env::Prod, &[], Fake::default());
    assert_eq!(message(&r), format!("no .aab under {}/build.", f.root.path().display()));
    assert!(t.runs().is_empty());
}

#[test]
fn an_unresolvable_package_is_named() {
    let f = fx(true, true);
    let (r, _) = go(&f, Env::Prod, &[("PLAY_PACKAGE_NAME", "")], Fake::default());
    assert_eq!(message(&r), "cannot resolve the Android package (set PLAY_PACKAGE_NAME).");
}

#[test]
fn a_missing_service_account_says_how_to_get_it() {
    let f = fx(true, false);
    let (r, t) = go(&f, Env::Prod, &[], Fake::default());
    assert_eq!(message(&r), format!("no Play service account at {} (dotfiles secret apply).", f.sa));
    assert!(t.runs().is_empty());
}

#[test]
fn the_service_account_defaults_under_home() {
    let f = fx(true, false);
    let (r, _) = go(&f, Env::Prod, &[("PLAY_SERVICE_ACCOUNT_PATH", "")], Fake::default());
    let home = std::env::temp_dir().join("deploy-fake-home-unused");
    assert!(message(&r).contains(&format!("{}/.config/dotfiles/play-service-account.json", home.display())), "{}", message(&r));
}

#[test]
fn a_failing_uploader_is_an_error() {
    let f = fx(true, true);
    let (r, _) = go(&f, Env::Prod, &[], Fake { failing: vec!["/py".into()], ..Fake::default() });
    assert!(r.is_err());
}
