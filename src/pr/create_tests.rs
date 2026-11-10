use super::*;
use crate::fakes::FakeForge;
use actautoma::forge::Mergeable;
use std::cell::RefCell;

#[test]
fn candidates_keep_the_base_like_branches_without_the_origin_prefix_sorted_and_unique() {
    let raw = "origin/feature/x\norigin/main\nmain\norigin/develop\norigin/release/1.0\norigin/dev\norigin/staging\norigin/master\nupstream/main\norigin\n";
    assert_eq!(candidates(raw), ["dev", "develop", "main", "master", "release/1.0", "staging"]);
}

#[test]
fn no_candidate_is_an_empty_list() {
    assert!(candidates("origin/feature/x\n").is_empty());
    assert!(candidates("").is_empty());
}

/// Scripted git, plus the scripted platform the pull request is created on.
struct Fake {
    calls: RefCell<Vec<String>>,
    branch: &'static str,
    remotes: &'static str,
    stat: &'static str,
    commit: &'static str,
    fail: Option<&'static str>,
    forge: FakeForge,
}

impl Fake {
    fn new() -> Self {
        Self { calls: RefCell::new(vec![]), branch: "feature", remotes: "origin/main\norigin/feature\n", stat: " a | 1 +\n", commit: "Last commit\n\nbody", fail: None, forge: FakeForge::pr("x", Mergeable::Mergeable) }
    }
    fn log(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    fn created(&self) -> Option<String> {
        self.forge.log().into_iter().find(|l| l.starts_with("create"))
    }
}

impl Exec for Fake {
    fn run(&self, prog: &str, args: &[&str], _: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        if self.fail.is_some_and(|p| line.starts_with(p)) {
            anyhow::bail!("{line} failed");
        }
        Ok(match args {
            ["branch", "--show-current"] => self.branch.to_string(),
            ["branch", "-r", ..] => self.remotes.to_string(),
            ["diff", "--stat", ..] => self.stat.to_string(),
            ["log", "-1", ..] => self.commit.to_string(),
            _ => String::new(),
        })
    }
}

fn go(opts: Opts, f: &Fake, input: &str) -> (Result<()>, String) {
    let mut out = Vec::new();
    let r = run(&opts, f, &f.forge, Path::new("."), &mut input.as_bytes(), &mut out);
    (r, String::from_utf8_lossy(&out).into_owned())
}

fn flags(base: &str, title: &str, body: &str) -> Opts {
    Opts { base: Some(base.into()), title: Some(title.into()), body: Some(body.into()) }
}

#[test]
fn everything_given_by_flags_creates_the_pr_without_asking() {
    let f = Fake::new();
    let (r, out) = go(flags("main", "T", "B"), &f, "");
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(f.created().as_deref(), Some("create --base main --head feature --title T --body B"));
    assert!(out.contains("Current branch: feature\n\n"), "{out}");
    assert!(out.contains("\nDiff against main:\n a | 1 +\n"), "{out}");
    assert!(out.contains("\nCreating pull request...\nhttps://github.com/o/r/pull/9\n"), "{out}");
    assert!(!out.contains("PR title"), "{out}");
}

#[test]
fn the_diff_is_taken_between_the_base_and_the_branch() {
    let f = Fake::new();
    let _ = go(flags("main", "T", "B"), &f, "");
    assert!(f.log().contains(&"git diff --stat main...feature".to_string()), "{:?}", f.log());
}

#[test]
fn main_and_master_are_refused_before_anything_is_asked() {
    for b in ["main", "master"] {
        let mut f = Fake::new();
        f.branch = b;
        let (r, _) = go(flags("main", "T", "B"), &f, "");
        let why = r.err().map(|e| e.to_string()).unwrap_or_default();
        assert_eq!(why, format!("You are already on {b}. Create a feature branch first."));
        assert!(f.created().is_none());
    }
}

#[test]
fn a_detached_head_is_refused() {
    let mut f = Fake::new();
    f.branch = "";
    let (r, _) = go(flags("main", "T", "B"), &f, "");
    assert_eq!(r.err().map(|e| e.to_string()).as_deref(), Some("Could not determine current branch."));
}

#[test]
fn the_single_base_candidate_is_chosen_and_said() {
    let f = Fake::new();
    let (r, out) = go(Opts { base: None, title: Some("T".into()), body: Some("B".into()) }, &f, "");
    assert!(r.is_ok(), "{r:?}");
    assert!(out.contains("Selected base branch: main\n"), "{out}");
    assert!(f.created().is_some_and(|c| c.contains("--base main")));
}

#[test]
fn several_candidates_without_a_base_fail_naming_them() {
    let mut f = Fake::new();
    f.remotes = "origin/main\norigin/develop\n";
    let (r, _) = go(Opts { base: None, title: Some("T".into()), body: Some("B".into()) }, &f, "");
    let why = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(why.contains("several base branches: develop, main") && why.contains("--base"), "{why}");
    assert!(f.created().is_none());
}

#[test]
fn no_candidate_without_a_base_fails_loudly() {
    let mut f = Fake::new();
    f.remotes = "origin/feature\n";
    let (r, _) = go(Opts { base: None, title: Some("T".into()), body: Some("B".into()) }, &f, "");
    assert!(r.err().is_some_and(|e| e.to_string().contains("no base branch found") && e.to_string().contains("--base")));
}

#[test]
fn a_missing_title_is_asked_and_an_empty_answer_takes_the_last_commit_subject() {
    let f = Fake::new();
    let (r, out) = go(Opts { base: Some("main".into()), title: None, body: Some("B".into()) }, &f, "\n");
    assert!(r.is_ok(), "{r:?}");
    assert!(out.contains("PR title (leave empty to use the last commit message): "), "{out}");
    assert!(out.contains("Using commit message: Last commit\n"), "{out}");
    assert!(f.created().is_some_and(|c| c.contains("--title Last commit --body B")), "{:?}", f.created());
}

#[test]
fn a_typed_title_is_used() {
    let f = Fake::new();
    let (_, _) = go(Opts { base: Some("main".into()), title: None, body: Some("B".into()) }, &f, "My title\n");
    assert!(f.created().is_some_and(|c| c.contains("--title My title ")));
}

#[test]
fn a_missing_body_is_read_until_the_end_of_input_one_line_each() {
    let f = Fake::new();
    let (_, out) = go(Opts { base: Some("main".into()), title: Some("T".into()), body: None }, &f, "line 1\nline 2\n");
    assert!(out.contains("PR description (press Ctrl-D to finish, empty allowed):\n"), "{out}");
    assert!(f.created().is_some_and(|c| c.ends_with("--body line 1\nline 2\n")), "{:?}", f.created());
}

#[test]
fn an_empty_body_is_allowed() {
    let f = Fake::new();
    let (r, _) = go(Opts { base: Some("main".into()), title: Some("T".into()), body: None }, &f, "");
    assert!(r.is_ok());
    assert!(f.created().is_some_and(|c| c.ends_with("--body ")));
}

#[test]
fn no_difference_with_the_base_is_refused_and_nothing_is_created() {
    let mut f = Fake::new();
    f.stat = "";
    let (r, _) = go(flags("main", "T", "B"), &f, "");
    assert_eq!(r.err().map(|e| e.to_string()).as_deref(), Some("No changes detected between main and feature."));
    assert!(f.created().is_none());
}

#[test]
fn a_failing_diff_is_an_error_not_an_empty_diff() {
    let mut f = Fake::new();
    f.fail = Some("git diff");
    let (r, _) = go(flags("nope", "T", "B"), &f, "");
    assert!(r.is_err());
    assert!(f.created().is_none());
}

#[test]
fn a_failing_platform_create_fails_loudly() {
    let mut f = Fake::new();
    f.forge = FakeForge::pr("x", Mergeable::Mergeable).failing("create");
    let (r, _) = go(flags("main", "T", "B"), &f, "");
    assert!(r.is_err());
}

#[test]
fn a_failing_branch_lookup_fails() {
    let mut f = Fake::new();
    f.fail = Some("git branch --show-current");
    let (r, _) = go(flags("main", "T", "B"), &f, "");
    assert!(r.is_err());
}
