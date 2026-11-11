use super::*;
use crate::run::fake::Fake;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn go(name: &str, verb: Option<&str>, args: &[&str], entries: &[Entry], r: &Fake) -> (Result<Outcome>, String, String) {
    let (mut out, mut err) = (vec![], vec![]);
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let o = run(name, verb, &args, entries, r, &mut out, &mut err);
    (o, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

fn project(makefile: &str) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("Makefile"), makefile));
    d
}

fn entry(name: &str, path: &Path) -> Entry {
    Entry { name: name.into(), path: path.to_path_buf() }
}

fn path_arg(d: &tempfile::TempDir) -> String {
    d.path().display().to_string()
}

#[test]
fn a_directory_path_runs_the_verb() {
    let d = project("build:\n");
    let r = Fake::exiting(0);
    let (o, out, err) = go(&path_arg(&d), Some("build"), &[], &[], &r);
    assert_eq!(ok(o), Outcome::Done);
    assert!(out.is_empty() && err.is_empty());
    assert_eq!(r.calls.borrow()[0].0, "make");
}

#[test]
fn a_registered_name_resolves_and_a_directory_wins_over_a_name() {
    let reg = project("viaregistry:\n");
    let here = project("viadir:\n");
    let r = Fake::exiting(0);
    ok(go("p", Some("viaregistry"), &[], &[entry("p", reg.path())], &r).0);
    assert_eq!(r.calls.borrow().len(), 1);
    let r = Fake::exiting(0);
    let name = path_arg(&here);
    let (o, ..) = go(&name, Some("viadir"), &[], &[entry(&name, reg.path())], &r);
    assert_eq!(ok(o), Outcome::Done);
}

#[test]
fn an_unknown_name_says_how_to_list_the_projects() {
    let e = go("zz", None, &[], &[], &Fake::exiting(0)).0.err().map(|e| e.to_string());
    let e = e.unwrap_or_default();
    assert!(e.starts_with("Unknown project: zz") && e.contains("repo list") && e.contains("repo scan"), "{e}");
}

#[test]
fn a_registered_path_that_is_gone_says_so() {
    let e = go("p", None, &[], &[entry("p", Path::new("/nonexistent/p"))], &Fake::exiting(0)).0.err().map(|e| e.to_string());
    let e = e.unwrap_or_default();
    assert!(e.contains("'p' no longer exists: /nonexistent/p") && e.contains("repo scan"), "{e}");
}

#[test]
fn without_a_verb_the_verbs_are_listed_one_per_line_and_nothing_runs() {
    let d = project("a:\nb:\n");
    let r = Fake::exiting(0);
    let (o, out, err) = go(&path_arg(&d), None, &[], &[], &r);
    assert_eq!(ok(o), Outcome::Done);
    assert_eq!(out, "a\nb\n");
    assert!(err.is_empty() && r.calls.borrow().is_empty());
}

#[test]
fn a_project_with_no_verb_at_all_is_an_error() {
    let d = ok(tempfile::tempdir());
    let e = go(&path_arg(&d), None, &[], &[], &Fake::exiting(0)).0.err().map(|e| e.to_string());
    assert!(e.unwrap_or_default().starts_with("No commands found for "));
}

#[test]
fn an_unknown_verb_is_reported_with_the_available_ones_and_fails_with_one() {
    let d = project("a:\nb:\n");
    let r = Fake::exiting(0);
    let name = path_arg(&d);
    let (o, out, err) = go(&name, Some("zz"), &[], &[], &r);
    assert_eq!(ok(o), Outcome::Failed(1));
    assert!(out.is_empty() && r.calls.borrow().is_empty());
    assert_eq!(err, format!("'zz' isn't a known command for {name}.\nAvailable:\na\nb\n"));
}

#[test]
fn an_unknown_verb_in_a_project_without_verbs_lists_none() {
    let d = ok(tempfile::tempdir());
    let name = path_arg(&d);
    let (o, _, err) = go(&name, Some("zz"), &[], &[], &Fake::exiting(0));
    assert_eq!(ok(o), Outcome::Failed(1));
    assert_eq!(err, format!("'zz' isn't a known command for {name}.\n"));
}

#[test]
fn a_failing_verb_keeps_its_exit_code_and_listing_warnings_are_prefixed() {
    let d = project("a:\ninclude missing.mk\n");
    let (o, _, err) = go(&path_arg(&d), Some("a"), &[], &[], &Fake::exiting(7));
    assert_eq!(ok(o), Outcome::Failed(7));
    let (_, out, err2) = go(&path_arg(&d), None, &[], &[], &Fake::exiting(0));
    assert_eq!(out, "a\n");
    assert!(err2.starts_with("warn: include ") && err.is_empty(), "{err2}");
}
