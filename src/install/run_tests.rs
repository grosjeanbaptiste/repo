use super::*;
use crate::audit::diag::fake::Fake;
use std::fs;
use std::path::PathBuf;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn fx() -> (tempfile::TempDir, PathBuf, Vec<Entry>) {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    ok(fs::write(root.join("Cargo.toml"), "[package]"));
    ok(fs::write(root.join("Cargo.lock"), ""));
    let entries = vec![Entry { name: "ss".into(), path: root.clone() }];
    (d, root, entries)
}

fn probe() -> Fake {
    Fake::new(&[])
        .answering("git rev-parse --verify HEAD", Ok("abc\n"))
        .answering("git --no-optional-locks status --porcelain", Ok(""))
        .answering("cargo --version", Ok("cargo 1\n"))
}

fn opts(package: Option<&str>, bin: Option<&str>, force: bool) -> Opts {
    Opts { name: "ss".into(), package: package.map(String::from), bin: bin.map(String::from), force }
}

fn go(o: &Opts, entries: &[Entry], probe: &Fake, live: &Fake) -> (Result<()>, String) {
    let mut out = Vec::new();
    let r = run(o, entries, probe, live, &mut out);
    (r, String::from_utf8_lossy(&out).into_owned())
}

fn live() -> Fake {
    Fake::new(&[]).answering("dotfiles heavy run -- cargo install", Ok(""))
}

#[test]
fn the_binary_is_installed_through_the_heavy_queue_and_confirmed() {
    let (_d, root, entries) = fx();
    let l = live();
    let (r, out) = go(&opts(None, Some("ss"), false), &entries, &probe(), &l);
    assert!(r.is_ok(), "{r:?}");
    let want = format!("dotfiles heavy run -- cargo install --path {} --locked --bin ss", root.display());
    assert_eq!(*l.calls.borrow(), [want]);
    assert_eq!(out, format!("Installed ss from {}\n", root.display()));
}

#[test]
fn without_bin_or_package_the_project_name_is_confirmed() {
    let (_d, root, entries) = fx();
    let l = live();
    let (_, out) = go(&opts(None, None, false), &entries, &probe(), &l);
    assert_eq!(*l.calls.borrow(), [format!("dotfiles heavy run -- cargo install --path {} --locked", root.display())]);
    assert_eq!(out, format!("Installed ss from {}\n", root.display()));
}

#[test]
fn force_is_passed_after_bin() {
    let (_d, root, entries) = fx();
    let l = live();
    let (r, _) = go(&opts(None, Some("ss"), true), &entries, &probe(), &l);
    assert!(r.is_ok(), "{r:?}");
    let want = format!("dotfiles heavy run -- cargo install --path {} --locked --bin ss --force", root.display());
    assert_eq!(*l.calls.borrow(), [want]);
}

#[test]
fn a_package_is_installed_from_its_member_directory_and_named_in_the_confirmation() {
    let (_d, root, entries) = fx();
    let meta = format!(
        r#"{{"packages":[{{"name":"ss","manifest_path":"{0}/Cargo.toml"}},{{"name":"ss-ui","manifest_path":"{0}/ui/Cargo.toml"}}]}}"#,
        root.display()
    );
    let leaked: &'static str = Box::leak(meta.into_boxed_str());
    let p = probe().answering("cargo metadata --no-deps --format-version 1 --locked", Ok(leaked));
    let l = live();
    let (r, out) = go(&opts(Some("ss-ui"), None, false), &entries, &p, &l);
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(*l.calls.borrow(), [format!("dotfiles heavy run -- cargo install --path {}/ui --locked", root.display())]);
    assert_eq!(out, format!("Installed ss-ui from {}\n", root.display()));
}

#[test]
fn an_unknown_package_fails_before_compiling() {
    let (_d, _root, entries) = fx();
    let p = probe().answering("cargo metadata", Ok(r#"{"packages":[]}"#));
    let l = live();
    let (r, out) = go(&opts(Some("nope"), None, false), &entries, &p, &l);
    assert!(r.err().is_some_and(|e| e.to_string().starts_with("no package nope in ")));
    assert!(l.calls.borrow().is_empty());
    assert!(out.is_empty());
}

#[test]
fn a_failing_metadata_is_an_error_naming_the_project() {
    let (_d, _root, entries) = fx();
    let p = probe().answering("cargo metadata", Err("no lockfile"));
    let l = live();
    let (r, _) = go(&opts(Some("x"), None, false), &entries, &p, &l);
    let msg = r.err().map(|e| format!("{e:#}")).unwrap_or_default();
    assert!(msg.contains("cargo metadata failed for ss") && msg.contains("no lockfile"), "{msg}");
    assert!(l.calls.borrow().is_empty());
}

#[test]
fn a_dirty_project_is_refused_before_anything_is_compiled() {
    let (_d, _root, entries) = fx();
    let p = probe().overriding("git --no-optional-locks status --porcelain", Ok("?? x\n"));
    let l = live();
    let (r, out) = go(&opts(None, None, false), &entries, &p, &l);
    assert!(r.err().is_some_and(|e| e.to_string().starts_with("project has uncommitted changes")));
    assert!(l.calls.borrow().is_empty() && out.is_empty());
}

#[test]
fn a_failing_install_is_an_error_naming_the_project_and_confirms_nothing() {
    let (_d, _root, entries) = fx();
    let l = Fake::new(&[]).answering("dotfiles heavy run", Err("exit 101"));
    let (r, out) = go(&opts(None, None, false), &entries, &probe(), &l);
    let msg = r.err().map(|e| format!("{e:#}")).unwrap_or_default();
    assert!(msg.contains("Cargo installation failed for ss") && msg.contains("exit 101"), "{msg}");
    assert!(out.is_empty());
}
