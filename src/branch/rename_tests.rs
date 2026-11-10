use super::*;
use crate::fakes::FakeForge;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

/// Scripted `Exec` plus the scripted platform: answers by the full command line, records every
/// call of both in one log, so their order can be asserted.
struct Script {
    answers: Vec<(&'static str, std::result::Result<String, String>)>,
    calls: Rc<RefCell<Vec<String>>>,
    forge: FakeForge,
}

impl Script {
    fn new(answers: Vec<(&'static str, std::result::Result<String, String>)>) -> Self {
        let calls = Rc::new(RefCell::new(vec![]));
        let forge = FakeForge::default().logging_to(calls.clone());
        Self { answers, calls, forge }
    }
    fn hosted_as(mut self, slug: &str, default: &str) -> Self {
        self.forge = FakeForge::default().hosting(slug, default).logging_to(self.calls.clone());
        self
    }
    fn failing_platform(mut self, operation: &'static str) -> Self {
        self.forge = FakeForge::default().failing(operation).logging_to(self.calls.clone());
        self
    }
    fn mutating(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .filter(|c| {
                !(c.starts_with("git branch --format")
                    || *c == "git remote"
                    || *c == "git ls-files"
                    || *c == "view_repo")
            })
            .cloned()
            .collect()
    }
}

impl Exec for Script {
    fn run(&self, prog: &str, args: &[&str], _cwd: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        for (key, answer) in &self.answers {
            if line == *key {
                return answer.clone().map_err(|e| anyhow::anyhow!(e));
            }
        }
        Ok(String::new())
    }
}

const BRANCHES: &str = "git branch --format=%(refname:short)";

fn opts(delete_old: bool) -> Opts {
    Opts { from: "master".into(), to: "main".into(), delete_old }
}

fn local_only() -> Script {
    Script::new(vec![(BRANCHES, Ok("master\nfeat\n".into())), ("git remote", Ok(String::new()))])
}

fn with_origin(default: &str) -> Script {
    Script::new(vec![(BRANCHES, Ok("master\n".into())), ("git remote", Ok("origin\n".into()))])
        .hosted_as("me/ss", default)
}

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_repo_without_origin_is_renamed_locally_and_the_platform_is_never_asked() {
    let (exec, d) = (local_only(), dir());
    let out = rename(&exec, &exec.forge, d.path(), &opts(false), false).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exec.mutating(), ["git branch -m master main"]);
    assert!(!exec.calls.borrow().iter().any(|c| c.contains("repo")), "{:?}", exec.calls.borrow());
    assert!(out.executed && !out.already);
}

#[test]
fn a_repo_with_origin_runs_the_whole_sequence_in_order() {
    let (exec, d) = (with_origin("master"), dir());
    rename(&exec, &exec.forge, d.path(), &opts(false), false).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        exec.mutating(),
        [
            "git branch -m master main",
            "git push -u origin main",
            "set_default_branch me/ss main",
            "git remote set-head origin -a",
        ]
    );
}

#[test]
fn delete_old_adds_the_remote_deletion() {
    let (exec, d) = (with_origin("master"), dir());
    rename(&exec, &exec.forge, d.path(), &opts(true), false).unwrap_or_else(|e| panic!("{e}"));
    assert!(exec.mutating().contains(&"git push origin --delete master".to_string()));
}

#[test]
fn a_dry_run_changes_nothing_and_returns_the_plan() {
    let (exec, d) = (with_origin("master"), dir());
    let out = rename(&exec, &exec.forge, d.path(), &opts(false), true).unwrap_or_else(|e| panic!("{e}"));
    assert!(exec.mutating().is_empty(), "{:?}", exec.mutating());
    assert_eq!(out.steps.len(), 4);
    assert!(!out.executed);
}

#[test]
fn an_already_renamed_repo_is_a_success_that_does_nothing() {
    let exec = Script::new(vec![(BRANCHES, Ok("main\n".into()))]);
    let out = rename(&exec, &exec.forge, dir().path(), &opts(false), false).unwrap_or_else(|e| panic!("{e}"));
    assert!(out.already && !out.executed);
    assert!(exec.mutating().is_empty());
}

#[test]
fn a_missing_source_branch_is_an_error_naming_it() {
    let exec = Script::new(vec![(BRANCHES, Ok("feat\n".into()))]);
    let err = rename(&exec, &exec.forge, dir().path(), &opts(false), false).err().unwrap_or_else(|| panic!("expected an error: {}", "missing")).to_string();
    assert!(err.contains("master"), "{err}");
    assert!(exec.mutating().is_empty());
}

#[test]
fn an_existing_target_branch_is_refused_when_the_source_also_exists() {
    let exec = Script::new(vec![(BRANCHES, Ok("master\nmain\n".into()))]);
    let err = rename(&exec, &exec.forge, dir().path(), &opts(false), false).err().unwrap_or_else(|| panic!("expected an error: {}", "exists")).to_string();
    assert!(err.contains("main") && err.contains("already"), "{err}");
    assert!(exec.mutating().is_empty());
}

#[test]
fn a_remote_default_that_is_not_the_source_is_refused_before_any_change() {
    let (exec, d) = (with_origin("develop"), dir());
    let err = rename(&exec, &exec.forge, d.path(), &opts(false), false).err().unwrap_or_else(|| panic!("expected an error: {}", "default")).to_string();
    assert!(err.contains("develop") && err.contains("master"), "{err}");
    assert!(exec.mutating().is_empty());
}

#[test]
fn invalid_names_stop_everything_before_any_command() {
    let exec = local_only();
    let bad = Opts { from: "master".into(), to: "-x".into(), delete_old: false };
    assert!(rename(&exec, &exec.forge, dir().path(), &bad, false).is_err());
    assert!(exec.calls.borrow().is_empty());
}

#[test]
fn a_failing_step_stops_the_run_and_says_what_was_already_done() {
    let mut exec = with_origin("master");
    exec.answers.push(("git push -u origin main", Err("denied".into())));
    let err = rename(&exec, &exec.forge, dir().path(), &opts(false), false).err().unwrap_or_else(|| panic!("expected an error: {}", "push")).to_string();
    assert!(err.contains("git push -u origin main"), "{err}");
    assert!(err.contains("denied"), "{err}");
    assert!(err.contains("already done: git branch -m master main"), "{err}");
    assert!(!exec.mutating().iter().any(|c| c.starts_with("set_default_branch")));
}

#[test]
fn a_failing_probe_is_an_error_not_a_silent_default() {
    let exec = Script::new(vec![(BRANCHES, Err("not a git repository".into()))]);
    let err = rename(&exec, &exec.forge, dir().path(), &opts(false), false).err().unwrap_or_else(|| panic!("expected an error: {}", "probe")).to_string();
    assert!(err.contains("not a git repository"), "{err}");
}

#[test]
fn files_that_still_say_the_old_name_are_listed_as_warnings() {
    let d = dir();
    fs::write(d.path().join("ci.yml"), "on:\n  push:\n    branches: [master]\n").unwrap_or_else(|e| panic!("{e}"));
    fs::write(d.path().join("notes.md"), "nothing here\n").unwrap_or_else(|e| panic!("{e}"));
    fs::write(d.path().join("Cargo.lock"), "master\n").unwrap_or_else(|e| panic!("{e}"));
    let mut exec = local_only();
    exec.answers.push(("git ls-files", Ok("ci.yml\nnotes.md\nCargo.lock\nbin.dat\n".into())));
    fs::write(d.path().join("bin.dat"), [0xff_u8, 0xfe, b'm', b'a', b's', b't', b'e', b'r']).unwrap_or_else(|e| panic!("{e}"));
    let out = rename(&exec, &exec.forge, d.path(), &opts(false), true).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
    assert!(out.warnings[0].starts_with("ci.yml:3:"), "{}", out.warnings[0]);
}

#[test]
fn a_word_is_matched_whole_so_masterpiece_is_not_a_reference() {
    assert!(mentions("branches: [master]", "master"));
    assert!(mentions("git checkout master", "master"));
    assert!(!mentions("a masterpiece", "master"));
    assert!(!mentions("grandmaster_x", "master"));
}

#[test]
fn a_failing_platform_lookup_is_an_error_and_changes_nothing() {
    let (exec, d) = (with_origin("master").failing_platform("view_repo"), dir());
    let err = rename(&exec, &exec.forge, d.path(), &opts(false), false).err().unwrap_or_else(|| panic!("expected an error: {}", "view_repo")).to_string();
    assert!(err.contains("view_repo failed"), "{err}");
    assert!(exec.mutating().is_empty(), "{:?}", exec.mutating());
}

#[test]
fn a_failing_default_branch_switch_says_what_was_already_done() {
    let (exec, d) = (with_origin("master").failing_platform("set_default_branch"), dir());
    let err = rename(&exec, &exec.forge, d.path(), &opts(true), false).err().unwrap_or_else(|| panic!("expected an error: {}", "switch")).to_string();
    assert!(err.contains("set default branch of me/ss to main"), "{err}");
    assert!(err.contains("already done: git branch -m master main, git push -u origin main"), "{err}");
    assert!(!exec.mutating().iter().any(|c| c.contains("--delete")), "{:?}", exec.mutating());
}

#[test]
fn the_default_branch_is_switched_before_the_old_one_is_deleted() {
    let (exec, d) = (with_origin("master"), dir());
    rename(&exec, &exec.forge, d.path(), &opts(true), false).unwrap_or_else(|e| panic!("{e}"));
    let log = exec.mutating();
    let at = |what: &str| log.iter().position(|c| c.starts_with(what));
    assert!(at("set_default_branch") < at("git push origin --delete"), "{log:?}");
}
