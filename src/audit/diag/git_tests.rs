use super::super::fake::Fake;
use super::super::{Level, fail, ok as okf, warn};
use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(d) = p.parent() {
        ok(fs::create_dir_all(d));
    }
    ok(fs::write(p, text));
}

#[test]
fn no_gitmodules_no_finding() {
    let (d, f) = (ok(tempfile::tempdir()), Fake::new(&[]));
    let has = f.has();
    assert_eq!(submodules(d.path(), &f.host(&has)), None);
    assert!(f.calls.borrow().is_empty());
}

#[test]
fn initialised_submodules_are_counted() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".gitmodules", "");
    let f = Fake::new(&[]).answering("git -C", Ok(" abc one (v1)\n def two (v2)\n"));
    let has = f.has();
    assert_eq!(submodules(d.path(), &f.host(&has)), Some(okf("submodules: 2 initialized")));
}

#[test]
fn uninitialised_submodules_warn_with_the_fix() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".gitmodules", "");
    let f = Fake::new(&[]).answering("git -C", Ok("-abc one\n def two\n-ghi three\n"));
    let has = f.has();
    assert_eq!(
        submodules(d.path(), &f.host(&has)),
        Some(warn("submodules: 2 of 3 uninitialized (run `git submodule update --init --recursive`)"))
    );
}

#[test]
fn a_failing_submodule_status_is_a_failure_not_zero_submodules() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".gitmodules", "");
    let f = Fake::new(&[]).answering("git -C", Err("not a git repository"));
    let has = f.has();
    let r = submodules(d.path(), &f.host(&has));
    assert!(matches!(&r, Some(x) if x.level == Level::Fail && x.msg.contains("not a git repository")), "{r:?}");
}

#[test]
fn lfs_needs_the_attribute() {
    let d = ok(tempfile::tempdir());
    let f = Fake::new(&[]);
    let has = f.has();
    assert_eq!(ok(lfs(d.path(), &f.host(&has))), None);
    put(d.path(), ".gitattributes", "*.png text\n");
    assert_eq!(ok(lfs(d.path(), &f.host(&has))), None);
}

#[test]
fn lfs_with_the_tool_is_ok_without_it_a_warning() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".gitattributes", "*.bin filter=lfs diff=lfs merge=lfs -text\n");
    let (with, without) = (Fake::new(&["git-lfs"]), Fake::new(&[]));
    let (h1, h2) = (with.has(), without.has());
    assert_eq!(ok(lfs(d.path(), &with.host(&h1))), Some(okf("git-lfs available for tracked patterns")));
    assert_eq!(
        ok(lfs(d.path(), &without.host(&h2))),
        Some(warn("git-lfs not installed but .gitattributes uses filter=lfs (suggest: `dotfiles add git-lfs`)"))
    );
}

fn hook(root: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    put(root, ".git/hooks/pre-commit", "#!/bin/sh\n");
    ok(fs::set_permissions(root.join(".git/hooks/pre-commit"), fs::Permissions::from_mode(mode)));
}

#[test]
fn precommit_without_a_config_is_silent() {
    let d = ok(tempfile::tempdir());
    let f = Fake::new(&["pre-commit"]);
    let has = f.has();
    assert_eq!(precommit(d.path(), &f.host(&has)), None);
}

#[test]
fn precommit_installed_and_wired_is_ok_for_both_config_names() {
    for cfg in [".pre-commit-config.yaml", ".pre-commit-config.yml"] {
        let d = ok(tempfile::tempdir());
        put(d.path(), cfg, "");
        hook(d.path(), 0o755);
        let f = Fake::new(&["pre-commit"]);
        let has = f.has();
        assert_eq!(precommit(d.path(), &f.host(&has)), Some(okf("pre-commit hook installed")), "{cfg}");
    }
}

#[test]
fn precommit_installed_but_not_wired_warns() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".pre-commit-config.yaml", "");
    hook(d.path(), 0o644); // present but not executable
    let f = Fake::new(&["pre-commit"]);
    let has = f.has();
    assert_eq!(
        precommit(d.path(), &f.host(&has)),
        Some(warn("pre-commit installed but hook not wired in this repo (run `pre-commit install`)"))
    );
}

#[test]
fn precommit_missing_tool_warns_even_with_a_hook() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".pre-commit-config.yaml", "");
    hook(d.path(), 0o755);
    let f = Fake::new(&[]);
    let has = f.has();
    assert_eq!(
        precommit(d.path(), &f.host(&has)),
        Some(warn(".pre-commit-config.yaml present but pre-commit not installed (suggest: `dotfiles add pre-commit`)"))
    );
}

#[test]
fn husky_needs_package_json_declaring_it() {
    let d = ok(tempfile::tempdir());
    assert_eq!(ok(husky(d.path())), None);
    put(d.path(), "package.json", r#"{"name":"x"}"#);
    assert_eq!(ok(husky(d.path())), None);
}

#[test]
fn husky_declared_with_hooks_is_ok_without_them_a_warning() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "package.json", r#"{"devDependencies":{"husky":"^9"}}"#);
    assert_eq!(
        ok(husky(d.path())),
        Some(warn("husky declared in package.json but .husky/_ missing (run `npx husky install` or your package manager equivalent)"))
    );
    ok(fs::create_dir_all(d.path().join(".husky/_")));
    assert_eq!(ok(husky(d.path())), Some(okf("husky hooks installed")));
}

#[test]
fn an_unreadable_package_json_is_an_error() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    put(d.path(), "package.json", "{}");
    let f = d.path().join("package.json");
    ok(fs::set_permissions(&f, fs::Permissions::from_mode(0o000)));
    let r = husky(d.path());
    ok(fs::set_permissions(&f, fs::Permissions::from_mode(0o644)));
    assert!(r.is_err());
    let _ = fail("unused");
}
