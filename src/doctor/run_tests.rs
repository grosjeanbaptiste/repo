use super::*;
use crate::audit::diag::fake::Fake;
use crate::fakes::FakeForge;
use actautoma::forge::Mergeable;
use anyhow::anyhow;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

type Audit = Box<dyn Fn(&mut dyn Write, &mut dyn Write) -> Result<()>>;

fn clean_audit() -> Audit {
    Box::new(|out, _| {
        ok(writeln!(out, "== auditing x ==\n  nothing"));
        Ok(())
    })
}

fn go(root: &Path, forge: &FakeForge, audit: &Audit) -> (Result<()>, String, String) {
    let exec = Fake::new(&[]).answering("git config", Err("exit status: 1"));
    let ports = Ports { exec: &exec, forge, cutoff: "2026-10-11T00:00:00Z".into(), audit: &**audit };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = run(root, &ports, &mut out, &mut err);
    (r, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

fn forge() -> FakeForge {
    FakeForge::pr("x", Mergeable::Mergeable).failing("available")
}

fn why(r: Result<()>) -> String {
    r.err().map(|e| format!("{e:#}")).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn the_report_has_the_five_sections_in_order_and_ends_on_a_blank_line() {
    let d = ok(tempfile::tempdir());
    let (r, out, err) = go(d.path(), &forge(), &clean_audit());
    assert!(r.is_ok(), "{r:?}");
    assert!(err.is_empty(), "{err}");
    let want = format!(
        "== project doctor: {}/ ==\n\n  == Git ==\n    (not a git repo)\n\n  == GitHub ==\n",
        d.path().display()
    );
    assert!(out.starts_with(&want.replace("/ ==", " ==")), "{out}");
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("missing {s} in {out}"));
    assert!(at("== Git ==") < at("== GitHub =="));
    assert!(at("== GitHub ==") < at("== Quality signals =="));
    assert!(at("== Quality signals ==") < at("== License =="));
    assert!(at("== License ==") < at("== Audit =="));
    assert!(out.ends_with("    nothing\n\n"), "{out}");
}

#[test]
fn the_license_section_carries_the_license_line() {
    let d = ok(tempfile::tempdir());
    let (_, out, _) = go(d.path(), &forge(), &clean_audit());
    assert!(out.contains("  == License ==\n    License:               NONE"), "{out}");
}

#[test]
fn the_audit_report_and_its_warnings_are_indented_into_its_section() {
    let d = ok(tempfile::tempdir());
    let audit: Audit = Box::new(|out, err| {
        ok(writeln!(out, "line one\nline two"));
        ok(writeln!(err, "warn: unreadable"));
        Ok(())
    });
    let (_, out, _) = go(d.path(), &forge(), &audit);
    assert!(out.contains("  == Audit ==\n    line one\n    line two\n    warn: unreadable\n"), "{out}");
}

#[test]
fn an_audit_that_finds_a_problem_prints_its_report_then_fails_with_its_message() {
    let d = ok(tempfile::tempdir());
    let audit: Audit = Box::new(|out, _| {
        ok(writeln!(out, "1 missing"));
        Err(anyhow!("audit: 1 variable(s) missing"))
    });
    let (r, out, _) = go(d.path(), &forge(), &audit);
    assert!(out.contains("    1 missing\n"), "{out}");
    assert_eq!(why(r), "doctor: audit failed (audit: 1 variable(s) missing)");
}

#[test]
fn a_failing_section_is_shown_in_place_and_the_others_still_run() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("LICENSE"), "x"));
    ok(fs::set_permissions(d.path().join("LICENSE"), fs::Permissions::from_mode(0o000)));
    let (r, out, _) = go(d.path(), &forge(), &clean_audit());
    assert!(out.contains("  == License ==\n    (error: "), "{out}");
    assert!(out.contains("== Audit =="), "{out}");
    assert_eq!(why(r), "doctor: 1 section(s) failed (License)");
}

#[test]
fn every_problem_is_named_in_the_final_error() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("LICENSE"), "x"));
    ok(fs::set_permissions(d.path().join("LICENSE"), fs::Permissions::from_mode(0o000)));
    let audit: Audit = Box::new(|_, _| Err(anyhow!("audit: boom")));
    let (r, _, _) = go(d.path(), &forge(), &audit);
    assert_eq!(why(r), "doctor: 1 section(s) failed (License); audit failed (audit: boom)");
}

#[test]
fn github_warnings_go_to_stderr_as_warn_lines() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join(".git")));
    let exec = Fake::new(&[])
        .answering("git config --get remote.origin.url", Ok("git@github.com:me/ss.git\n"))
        .answering("git --no-optional-locks status", Ok(""))
        .answering("git", Err("nothing"));
    let f = FakeForge::pr("x", Mergeable::Mergeable).failing("open_prs");
    let audit = clean_audit();
    let ports = Ports { exec: &exec, forge: &f, cutoff: "2026-10-11T00:00:00Z".into(), audit: &*audit };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    ok(run(d.path(), &ports, &mut out, &mut err));
    let err = String::from_utf8_lossy(&err).into_owned();
    assert!(err.starts_with("warn: doctor: cannot list open PRs"), "{err}");
}
