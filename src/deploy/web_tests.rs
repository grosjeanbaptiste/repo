use super::*;
use crate::deploy::fake::Fake;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn root_with(dirs: &[&str]) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    for x in dirs {
        ok(fs::create_dir_all(d.path().join(x)));
    }
    d
}

fn git(branch: Option<&str>) -> Fake {
    Fake { captures: vec![("git".into(), "rev-parse".into(), branch.map(String::from))], ..Fake::default() }
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

#[test]
fn the_first_existing_candidate_wins_in_priority_order() {
    for (i, want) in CANDIDATES.iter().enumerate() {
        let later: Vec<&str> = CANDIDATES[i..].to_vec();
        let d = root_with(&later);
        let got = ok(resolve_dir(d.path()));
        assert_eq!(got, ok(d.path().join(want).canonicalize()));
    }
}

#[test]
fn a_file_named_like_a_candidate_is_not_a_build_dir() {
    let d = root_with(&["dist"]);
    ok(fs::write(d.path().join("build"), "x"));
    ok(fs::remove_dir(d.path().join("dist")));
    assert!(resolve_dir(d.path()).is_err());
}

#[test]
fn no_build_output_says_where_it_looked() {
    let d = root_with(&[]);
    let e = resolve_dir(d.path()).err().map(|e| e.to_string()).unwrap_or_default();
    assert_eq!(e, format!("No web build output under {} (looked for: build/web dist build out public). Build first.", d.path().display()));
}

#[test]
fn prod_always_deploys_to_main_without_asking_git() {
    let d = root_with(&[]);
    let t = git(Some("feat/x"));
    assert_eq!(ok(branch(Env::Prod, d.path(), &t)), "main");
    assert!(t.runs().is_empty());
}

#[test]
fn test_uses_the_current_git_branch() {
    let d = root_with(&[]);
    assert_eq!(ok(branch(Env::Test, d.path(), &git(Some("feat/x")))), "feat/x");
}

#[test]
fn test_falls_back_to_preview_when_git_has_no_branch() {
    let d = root_with(&[]);
    for out in [None, Some(String::new()), Some("HEAD".to_string())] {
        let t = Fake { captures: vec![("git".into(), "rev-parse".into(), out)], ..Fake::default() };
        assert_eq!(ok(branch(Env::Test, d.path(), &t)), "preview");
    }
}

#[test]
fn wrangler_args_carry_dir_project_and_branch() {
    assert_eq!(wrangler_args(Path::new("/p/dist"), "app", "main"), s(&["pages", "deploy", "/p/dist", "--project-name=app", "--branch=main"]));
}

#[test]
fn deploy_runs_wrangler_with_the_project_directory_name_by_default() {
    let d = root_with(&["dist"]);
    let t = Fake::default();
    let mut out = vec![];
    ok(deploy(Env::Prod, d.path(), None, false, &t, &mut out));
    let name = d.path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dist = ok(d.path().join("dist").canonicalize());
    assert_eq!(t.lines(), vec![format!("wrangler pages deploy {} --project-name={name} --branch=main", dist.display())]);
    assert!(out.is_empty());
}

#[test]
fn an_explicit_project_name_wins() {
    let d = root_with(&["dist"]);
    let t = Fake::default();
    ok(deploy(Env::Prod, d.path(), Some("site"), false, &t, &mut vec![]));
    assert!(t.lines()[0].contains("--project-name=site"));
}

#[test]
fn dry_run_prints_the_command_and_runs_nothing() {
    let d = root_with(&["dist"]);
    let t = Fake::default();
    let mut out = vec![];
    ok(deploy(Env::Prod, d.path(), Some("site"), true, &t, &mut out));
    let dist = ok(d.path().join("dist").canonicalize());
    assert_eq!(String::from_utf8_lossy(&out), format!("wrangler pages deploy {} --project-name=site --branch=main\n", dist.display()));
    assert!(t.runs().is_empty());
}

#[test]
fn a_missing_wrangler_is_refused_with_the_install_hint_before_anything() {
    let d = root_with(&[]);
    let t = Fake { missing: vec!["wrangler".into()], ..Fake::default() };
    let e = deploy(Env::Prod, d.path(), None, false, &t, &mut vec![]).err().map(|e| e.to_string()).unwrap_or_default();
    assert_eq!(e, "wrangler not found\nInstall it with: dotfiles add --manager npm wrangler");
}

#[test]
fn a_failing_wrangler_is_an_error() {
    let d = root_with(&["dist"]);
    let t = Fake { failing: vec!["wrangler".into()], ..Fake::default() };
    assert!(deploy(Env::Prod, d.path(), None, false, &t, &mut vec![]).is_err());
}

#[test]
fn git_is_asked_inside_the_project() {
    let d = root_with(&[]);
    let t = git(Some("feat/x"));
    ok(branch(Env::Test, d.path(), &t));
    let asked = t.captured.borrow().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!((asked[0].prog.as_str(), asked[0].args.clone()), ("git", s(&["rev-parse", "--abbrev-ref", "HEAD"])));
    assert_eq!(asked[0].cwd.as_deref(), Some(d.path()));
}
