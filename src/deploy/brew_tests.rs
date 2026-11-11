use super::*;
use crate::deploy::fake::{Fake, drive, message};
use crate::deploy::target::Env;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn project(casks: bool, version: Option<&str>) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    if casks {
        ok(fs::create_dir_all(d.path().join("stores/brew/Casks")));
    }
    if let Some(v) = version {
        ok(fs::write(d.path().join("pubspec.yaml"), format!("name: a\nversion: {v}\n")));
    }
    d
}

fn go(root: &std::path::Path, env: Env, extra: &[(&str, &str)], tool: Fake) -> (Result<()>, Fake) {
    let pairs = [&[("BREW_APP_URL", "https://h/app.zip"), ("STORES_PYTHON", "/py"), ("STORES_SCRIPTS_DIR", "/s")][..], extra].concat();
    drive(env, root, &pairs, tool, update)
}

#[test]
fn prod_bumps_the_stable_cask_with_the_pubspec_version() {
    let d = project(true, Some("1.2.3+9"));
    let (r, t) = go(d.path(), Env::Prod, &[], Fake::default());
    assert!(r.is_ok(), "{}", message(&r));
    let brew = d.path().join("stores/brew");
    assert_eq!(
        t.lines(),
        vec![format!("/py /s/update_brew.py --version 1.2.3 --app-url https://h/app.zip --brew-root {} --channel stable", brew.display())]
    );
}

#[test]
fn test_bumps_the_beta_cask() {
    let d = project(true, Some("1.0.0"));
    let (_, t) = go(d.path(), Env::Test, &[], Fake::default());
    assert!(t.lines()[0].ends_with("--channel beta"));
}

#[test]
fn the_version_and_brew_root_can_be_overridden() {
    let d = project(false, None);
    ok(fs::create_dir_all(d.path().join("elsewhere/Formula")));
    let root = d.path().join("elsewhere");
    let (r, t) = go(d.path(), Env::Prod, &[("BREW_VERSION", "7.0.0"), ("BREW_ROOT_DIR", root.to_str().unwrap_or_default())], Fake::default());
    assert!(r.is_ok(), "{}", message(&r));
    assert!(t.lines()[0].contains(&format!("--version 7.0.0 --app-url https://h/app.zip --brew-root {}", root.display())));
}

#[test]
fn a_missing_hosted_url_is_named_before_anything() {
    let d = project(true, Some("1.0.0"));
    let (r, t) = go(d.path(), Env::Prod, &[("BREW_APP_URL", "")], Fake::default());
    assert_eq!(message(&r), "set BREW_APP_URL to the hosted macOS artifact (Homebrew references a URL, not a local file).");
    assert!(t.runs().is_empty());
}

#[test]
fn a_brew_root_without_casks_or_formulae_is_refused() {
    let d = project(false, Some("1.0.0"));
    let (r, _) = go(d.path(), Env::Prod, &[], Fake::default());
    assert_eq!(message(&r), format!("no cask/formula under {}/stores/brew (scaffold stores/brew).", d.path().display()));
}

#[test]
fn a_formula_folder_alone_is_enough() {
    let d = project(false, Some("1.0.0"));
    ok(fs::create_dir_all(d.path().join("stores/brew/Formula")));
    assert!(go(d.path(), Env::Prod, &[], Fake::default()).0.is_ok());
}

#[test]
fn an_unresolvable_version_is_named() {
    let d = project(true, None);
    let (r, _) = go(d.path(), Env::Prod, &[], Fake::default());
    assert_eq!(message(&r), "cannot resolve version (set BREW_VERSION).");
}

#[test]
fn a_failing_updater_is_an_error() {
    let d = project(true, Some("1.0.0"));
    assert!(go(d.path(), Env::Prod, &[], Fake { failing: vec!["/py".into()], ..Fake::default() }).0.is_err());
}
