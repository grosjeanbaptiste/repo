use super::*;
use crate::fakes::{Recorder, Registry};

fn opts(source: Source, root: &Path) -> NewOpts {
    NewOpts { kind: "package/rust".into(), source, root: root.to_path_buf(), no_push: true, dry_run: false }
}

fn named(name: &str) -> Source {
    Source::Named(name.into())
}

fn query() -> Query {
    Query { prefixes: vec!["act".into()], domain: "git repositories tooling".into(), max_length: 10, limit: 20 }
}

fn root() -> tempfile::TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn ifdck_goes_through_the_whole_pipeline() {
    let (dir, scaffold) = (root(), Recorder::default());
    let name = run(&opts(named("ifdck"), dir.path()), &Registry::free(), &scaffold);
    assert_eq!(name.ok().as_deref(), Some("ifdck"));
    let root = dir.path().display().to_string();
    let expected: Vec<String> = ["setup", "package/rust", "ifdck", "--path", &root, "--no-push"]
        .map(String::from)
        .to_vec();
    assert_eq!(scaffold.calls.borrow().as_slice(), [expected]);
}

#[test]
fn push_is_left_to_dotfiles_unless_no_push() {
    let (dir, scaffold) = (root(), Recorder::default());
    let o = NewOpts { no_push: false, ..opts(named("ifdck"), dir.path()) };
    assert!(run(&o, &Registry::free(), &scaffold).is_ok());
    assert!(!scaffold.calls.borrow()[0].contains(&"--no-push".to_string()));
}

#[test]
fn a_name_used_on_github_stops_before_any_scaffold() {
    let (dir, scaffold) = (root(), Recorder::default());
    let err = run(&opts(named("ifdck"), dir.path()), &Registry::taking(&["ifdck"]), &scaffold);
    assert!(err.err().is_some_and(|e| e.to_string().contains("ifdck is taken")));
    assert!(scaffold.calls.borrow().is_empty());
}

#[test]
fn an_existing_directory_stops_before_any_scaffold() {
    let (dir, scaffold) = (root(), Recorder::default());
    assert!(std::fs::create_dir(dir.path().join("ifdck")).is_ok());
    assert!(run(&opts(named("ifdck"), dir.path()), &Registry::free(), &scaffold).is_err());
    assert!(scaffold.calls.borrow().is_empty());
}

#[test]
fn github_failure_aborts_without_scaffolding() {
    let (dir, scaffold) = (root(), Recorder::default());
    let err = run(&opts(named("ifdck"), dir.path()), &Registry::failing("rate limit"), &scaffold);
    assert!(err.err().is_some_and(|e| e.to_string().contains("rate limit")));
    assert!(scaffold.calls.borrow().is_empty());
}

#[test]
fn scaffold_failure_surfaces() {
    let dir = root();
    let scaffold = Recorder { fail: true, ..Recorder::default() };
    assert!(run(&opts(named("ifdck"), dir.path()), &Registry::free(), &scaffold).is_err());
}

#[test]
fn dry_run_names_but_scaffolds_nothing() {
    let (dir, scaffold) = (root(), Recorder::default());
    let o = NewOpts { dry_run: true, ..opts(named("ifdck"), dir.path()) };
    assert_eq!(run(&o, &Registry::free(), &scaffold).ok().as_deref(), Some("ifdck"));
    assert!(scaffold.calls.borrow().is_empty());
}

#[test]
fn generated_name_skips_taken_candidates() {
    let (dir, scaffold) = (root(), Recorder::default());
    let all = naming::candidates(&query()).unwrap_or_default();
    assert!(all.len() > 1, "{all:?}");
    let registry = Registry::taking(&[all[0].as_str()]);
    let name = run(&opts(Source::Generated(query()), dir.path()), &registry, &scaffold);
    assert_eq!(name.ok().as_deref(), Some(all[1].as_str()));
    assert_eq!(scaffold.calls.borrow()[0][2], all[1]);
}

fn refused(name: &str, dir: &tempfile::TempDir) -> (String, usize) {
    let scaffold = Recorder::default();
    let err = run(&opts(named(name), dir.path()), &Registry::free(), &scaffold);
    (err.err().map(|e| e.to_string()).unwrap_or_default(), scaffold.calls.borrow().len())
}

#[test]
fn names_that_are_not_one_path_component_are_refused_as_invalid() {
    let dir = root();
    for bad in ["..", ".", "../x", "a/b", "/abs", ""] {
        let (message, calls) = refused(bad, &dir);
        assert!(message.contains("not a valid"), "{bad:?}: {message}");
        assert_eq!(calls, 0, "{bad:?}");
    }
}

#[test]
fn names_outside_the_github_charset_are_refused() {
    let dir = root();
    for bad in ["a b", "ifdck!", "if\tdck", "ifdck\n"] {
        let (message, calls) = refused(bad, &dir);
        assert!(message.contains("not a valid"), "{bad:?}: {message}");
        assert_eq!(calls, 0, "{bad:?}");
    }
}

#[test]
fn when_every_generated_candidate_is_taken_nothing_is_scaffolded() {
    let (dir, scaffold) = (root(), Recorder::default());
    let all = naming::candidates(&query()).unwrap_or_default();
    let refs: Vec<&str> = all.iter().map(String::as_str).collect();
    let err = run(&opts(Source::Generated(query()), dir.path()), &Registry::taking(&refs), &scaffold);
    assert!(err.err().is_some_and(|e| e.to_string().contains("no free name")));
    assert!(scaffold.calls.borrow().is_empty());
}

#[test]
fn generated_name_skips_a_candidate_whose_directory_exists() {
    let (dir, scaffold) = (root(), Recorder::default());
    let all = naming::candidates(&query()).unwrap_or_default();
    assert!(std::fs::create_dir(dir.path().join(&all[0])).is_ok());
    let name = run(&opts(Source::Generated(query()), dir.path()), &Registry::free(), &scaffold);
    assert_eq!(name.ok().as_deref(), Some(all[1].as_str()));
}

#[test]
fn actname_refusal_aborts_before_any_scaffold() {
    let (dir, scaffold) = (root(), Recorder::default());
    let no_prefix = Query { prefixes: vec![], ..query() };
    let err = run(&opts(Source::Generated(no_prefix), dir.path()), &Registry::free(), &scaffold);
    assert!(err.err().is_some_and(|e| e.to_string().contains("actname")));
    assert!(scaffold.calls.borrow().is_empty());
}

#[test]
fn kind_and_a_root_with_spaces_reach_dotfiles_verbatim() {
    let dir = root();
    let spaced = dir.path().join("my projects");
    assert!(std::fs::create_dir(&spaced).is_ok());
    let scaffold = Recorder::default();
    let o = NewOpts { kind: "app/python".into(), ..opts(named("ifdck"), &spaced) };
    assert!(run(&o, &Registry::free(), &scaffold).is_ok());
    let call = scaffold.calls.borrow()[0].clone();
    assert_eq!((call[1].as_str(), call[4].as_str()), ("app/python", spaced.to_str().unwrap_or("")));
}
