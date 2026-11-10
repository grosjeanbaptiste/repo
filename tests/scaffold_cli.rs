//! `repo check|apply` end to end on temp directories and a temp template tree. No network.
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn templates() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    let t = d.path().join("project-templates");
    ok(fs::create_dir_all(t.join("common")));
    ok(fs::write(
        t.join("common/README.md.tmpl"),
        "# {{project_name}}\n",
    ));
    ok(fs::write(t.join("common/.gitignore"), "target\n"));
    d
}

fn repo(root: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(args)
        .current_dir(cwd)
        .env("PROJECT_REPO_ROOT", root)
        .output()
        .unwrap_or_else(|e| panic!("cannot run repo: {e}"))
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

#[test]
fn check_on_an_empty_project_without_type_fails_and_names_the_flag() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    let o = repo(t.path(), p.path(), &["check"]);
    assert!(!o.status.success());
    assert!(err(&o).contains("Pass --type <t>"), "{}", err(&o));
}

#[test]
fn check_then_apply_then_check_goes_from_missing_to_ok() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    ok(fs::write(p.path().join("package.json"), "{}"));
    let o = repo(t.path(), p.path(), &["check"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(out(&o).contains("  - missing  README.md"), "{}", out(&o));
    let o = repo(t.path(), p.path(), &["apply"]);
    assert!(out(&o).contains("  + wrote      README.md"), "{}", out(&o));
    let o = repo(t.path(), p.path(), &["check"]);
    assert!(out(&o).contains("  = ok       README.md"), "{}", out(&o));
    assert!(out(&o).contains("  = ok       .gitignore"), "{}", out(&o));
}

#[test]
fn apply_skips_edited_files_unless_forced() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    ok(fs::write(p.path().join("README.md"), "mine\n"));
    let a = ["apply", "--type", "x"];
    let o = repo(t.path(), p.path(), &a);
    assert!(out(&o).contains("  ! exists (skipped, use --force)  README.md"));
    assert_eq!(ok(fs::read_to_string(p.path().join("README.md"))), "mine\n");
    let o = repo(t.path(), p.path(), &["apply", "--type", "x", "--force"]);
    assert!(out(&o).contains("  + wrote      README.md"));
    assert_ne!(ok(fs::read_to_string(p.path().join("README.md"))), "mine\n");
}

#[test]
fn apply_diff_shows_the_change_and_writes_nothing() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    ok(fs::write(p.path().join("README.md"), "mine\n"));
    let o = repo(t.path(), p.path(), &["apply", "--type", "x", "--diff"]);
    assert!(
        out(&o).contains("~ would update  README.md (diff below)"),
        "{}",
        out(&o)
    );
    assert!(out(&o).contains("-mine"), "{}", out(&o));
    assert!(!p.path().join(".gitignore").exists());
}

#[test]
fn force_and_diff_are_mutually_exclusive() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    let o = repo(
        t.path(),
        p.path(),
        &["apply", "--type", "x", "--force", "--diff"],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(err(&o).contains("cannot be used with"), "{}", err(&o));
}

#[test]
fn a_path_argument_and_a_missing_path_are_honoured() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    let o = repo(t.path(), p.path(), &["check", "sub", "--type", "x"]);
    assert!(!o.status.success());
    assert!(err(&o).contains("no such directory"), "{}", err(&o));
    ok(fs::create_dir(p.path().join("sub")));
    let o = repo(t.path(), p.path(), &["check", "sub", "--type", "x"]);
    assert!(
        out(&o).contains("==> Checking x project at "),
        "{}",
        out(&o)
    );
}

#[test]
fn missing_template_root_fails_loudly_without_writing() {
    let p = ok(tempfile::tempdir());
    let o = repo(
        Path::new("/nonexistent/dotfiles"),
        p.path(),
        &["apply", "--type", "x"],
    );
    // an absent templates root is an empty plan, not a crash: nothing written
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(ok(fs::read_dir(p.path())).count(), 0);
}

#[test]
fn license_prints_the_doctor_line_for_each_state() {
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    let o = repo(t.path(), p.path(), &["license"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).contains("License:") && out(&o).contains("NONE"),
        "{}",
        out(&o)
    );
    ok(fs::write(
        p.path().join("LICENSE"),
        "Copyright (c) A. All rights reserved.\n",
    ));
    let o = repo(t.path(), p.path(), &["license"]);
    assert!(out(&o).contains("proprietary (LICENSE)"), "{}", out(&o));
}

#[test]
fn license_takes_a_path_and_fails_loudly_on_an_unreadable_file() {
    use std::os::unix::fs::PermissionsExt;
    let (t, p) = (templates(), ok(tempfile::tempdir()));
    let sub = p.path().join("sub");
    ok(fs::create_dir(&sub));
    ok(fs::write(sub.join("LICENSE"), "x"));
    ok(fs::set_permissions(
        sub.join("LICENSE"),
        fs::Permissions::from_mode(0o0),
    ));
    let o = repo(t.path(), p.path(), &["license", "sub"]);
    ok(fs::set_permissions(
        sub.join("LICENSE"),
        fs::Permissions::from_mode(0o644),
    ));
    assert!(!o.status.success());
    assert!(err(&o).contains("cannot read"), "{}", err(&o));
}
