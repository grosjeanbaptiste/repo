use super::*;
use crate::deploy::fake::Fake;
use crate::deploy::target::Target;
use std::fs;
use std::path::Path;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

struct Out {
    result: Result<()>,
    tool: Fake,
    out: String,
    err: String,
}

fn opts(path: &Path, choice: Choice) -> Opts {
    Opts { env: Env::Prod, choice, path: path.to_path_buf(), project: None, yes: true, dry_run: false, store_page: false }
}

fn go(o: &Opts, answer: &str, tool: Fake) -> Out {
    let vars = Vars::of(&[("STORES_PYTHON", "/py"), ("STORES_SCRIPTS_DIR", "/s")], &o.path.join("home"));
    let (mut out, mut err) = (vec![], vec![]);
    let result = run(o, &vars, &tool, &mut answer.as_bytes(), &mut out, &mut err);
    Out { result, tool, out: String::from_utf8_lossy(&out).into_owned(), err: String::from_utf8_lossy(&err).into_owned() }
}

fn msg(o: &Out) -> String {
    o.result.as_ref().err().map(|e| format!("{e}")).unwrap_or_default()
}

fn project() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir_all(d.path().join("dist")));
    d
}

fn shown(d: &tempfile::TempDir) -> String {
    ok(d.path().canonicalize()).display().to_string()
}

#[test]
fn a_path_that_is_not_a_directory_is_refused() {
    let o = go(&opts(Path::new("/nonexistent/p"), Choice::All), "", Fake::default());
    assert_eq!(msg(&o), "not a directory: /nonexistent/p");
    assert!(o.tool.runs().is_empty());
}

#[test]
fn a_confirmed_deploy_asks_with_the_target_env_and_path_then_runs() {
    for yes in ["y", "Y", "yes", "YES", "y\n"] {
        let d = project();
        let mut p = opts(d.path(), Choice::One(Target::Web));
        p.yes = false;
        let o = go(&p, yes, Fake::default());
        assert!(o.result.is_ok(), "{yes}: {}", msg(&o));
        assert_eq!(o.out, format!("Deploy 'web' to 'prod' from {}? [y/N] ", shown(&d)));
        assert_eq!(o.tool.runs().len(), 1);
    }
}

#[test]
fn anything_else_aborts_and_runs_nothing() {
    for answer in ["", "n", "N", "no", "maybe", "\n", "yy"] {
        let d = project();
        let mut p = opts(d.path(), Choice::One(Target::Web));
        p.yes = false;
        let o = go(&p, answer, Fake::default());
        assert_eq!(msg(&o), "Aborted.", "{answer:?}");
        assert!(o.tool.runs().is_empty());
    }
}

#[test]
fn yes_and_dry_run_skip_the_question() {
    let d = project();
    let mut p = opts(d.path(), Choice::One(Target::Web));
    let o = go(&p, "n", Fake::default());
    assert!(o.result.is_ok() && !o.out.contains("[y/N]"));
    p.yes = false;
    p.dry_run = true;
    let o = go(&p, "n", Fake::default());
    assert!(o.result.is_ok() && !o.out.contains("[y/N]"), "{}", o.out);
    assert!(o.tool.runs().is_empty());
}

#[test]
fn the_project_name_reaches_wrangler() {
    let d = project();
    let mut p = opts(d.path(), Choice::One(Target::Web));
    p.project = Some("site".into());
    assert!(go(&p, "", Fake::default()).tool.lines()[0].contains("--project-name=site"));
}

#[test]
fn all_plans_every_target_in_order_on_a_dry_run() {
    let d = project();
    let mut p = opts(d.path(), Choice::All);
    p.dry_run = true;
    let o = go(&p, "", Fake::default());
    assert!(o.result.is_ok(), "{}", msg(&o));
    let starts: Vec<&str> = o.out.lines().map(|l| l.split(' ').next().unwrap_or_default()).collect();
    assert_eq!(starts, ["wrangler", "flutter", "flutter", "macOS", "flutter", "flutter", "flutter"]);
    assert!(o.tool.runs().is_empty());
}

#[test]
fn all_goes_on_after_a_failure_reports_each_and_fails_at_the_end() {
    let d = ok(tempfile::tempdir());
    let o = go(&opts(d.path(), Choice::All), "", Fake { failing: vec!["flutter".into()], ..Fake::default() });
    assert_eq!(msg(&o), "deploy: 7 of 7 target(s) failed (web, ios, android, mas, homebrew, windows, linux)");
    for t in ["web", "ios", "android", "mas", "homebrew", "windows", "linux"] {
        assert!(o.err.lines().any(|l| l.starts_with(&format!("deploy {t}: "))), "{t}: {}", o.err);
    }
    assert_eq!(o.tool.runs().iter().filter(|r| r.prog == "flutter").count(), 4 + 1, "ios android homebrew windows linux built; mas never started");
}

#[test]
fn all_succeeds_when_only_some_targets_are_in_play_and_none_fail() {
    let d = project();
    let mut p = opts(d.path(), Choice::All);
    p.dry_run = true;
    ok(fs::remove_dir_all(d.path().join("dist")));
    let o = go(&p, "", Fake::default());
    assert_eq!(msg(&o), "deploy: 1 of 7 target(s) failed (web)");
    assert_eq!(o.out.lines().count(), 6, "the six native plans still print");
}

#[test]
fn one_failing_target_is_that_error_prefixed_with_its_name() {
    let d = ok(tempfile::tempdir());
    let o = go(&opts(d.path(), Choice::One(Target::Web)), "", Fake::default());
    assert!(msg(&o).starts_with("deploy web: No web build output under "), "{}", msg(&o));
    assert!(o.err.is_empty(), "reported once, by the caller: {}", o.err);
}

#[test]
fn store_page_confirms_in_its_own_words_and_never_builds() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir_all(d.path().join("stores")));
    let mut p = opts(d.path(), Choice::One(Target::Ios));
    p.store_page = true;
    p.yes = false;
    p.dry_run = false;
    let vars = Vars::of(&[("STORES_PYTHON", "/py"), ("STORES_SCRIPTS_DIR", "/s"), ("ASC_BUNDLE_ID", "b")], d.path());
    let tool = Fake::default();
    let (mut out, mut err) = (vec![], vec![]);
    ok(run(&p, &vars, &tool, &mut "y\n".as_bytes(), &mut out, &mut err));
    assert_eq!(String::from_utf8_lossy(&out), format!("Push the ios store page from {}/stores? [y/N] ", shown_path(d.path())));
    assert_eq!(tool.runs().len(), 1);
    assert_eq!(tool.runs()[0].args, ["/s/asc_upload.py"]);
}

fn shown_path(p: &Path) -> String {
    ok(p.canonicalize()).display().to_string()
}

#[test]
fn store_page_for_all_or_web_fails_after_the_question_like_bash() {
    let d = ok(tempfile::tempdir());
    let mut p = opts(d.path(), Choice::All);
    p.store_page = true;
    let o = go(&p, "", Fake::default());
    assert_eq!(msg(&o), "--store-page applies to ios and android, not 'all'.");
}
