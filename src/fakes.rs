//! Test doubles for the two seams: GitHub (`RepoNames`) and `dotfiles` (`Scaffold`).
use crate::pipeline::Scaffold;
use actautoma::forge::{Error as ForgeError, Forge, Mergeable, NewPullRequest, PullRequest};
use actit::contains::Listing;
use actit::error::Error;
use actit::ports::RepoNames;
use anyhow::Result;
use std::cell::{Cell, RefCell};

pub struct Registry {
    taken: Vec<String>,
    failure: Option<Error>,
    /// Searches still answered before `failure` starts (0 = fail at once).
    grace: Cell<usize>,
}

impl Registry {
    pub fn free() -> Self {
        Self {
            taken: vec![],
            failure: None,
            grace: Cell::new(0),
        }
    }

    pub fn taking(names: &[&str]) -> Self {
        Self {
            taken: names.iter().map(|s| s.to_string()).collect(),
            failure: None,
            grace: Cell::new(0),
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            taken: vec![],
            failure: Some(Error::Github(message.into())),
            grace: Cell::new(0),
        }
    }

    /// Answers `n` searches (naming `taken` as used), then GitHub fails:
    /// a rate limit in the middle of a run.
    pub fn failing_after(taken: &[&str], n: usize, message: &str) -> Self {
        Self {
            grace: Cell::new(n),
            ..Self {
                failure: Some(Error::Github(message.into())),
                ..Self::taking(taken)
            }
        }
    }
}

impl RepoNames for Registry {
    fn owner(&self) -> Result<String, Error> {
        Ok("me".into())
    }
    fn exists(&self, _: &str, _: &str) -> Result<bool, Error> {
        Ok(false)
    }
    fn search(&self, name: &str) -> Result<Vec<String>, Error> {
        let grace = self.grace.get();
        match &self.failure {
            Some(e) if grace == 0 => Err(e.clone()),
            _ => {
                self.grace.set(grace.saturating_sub(1));
                let hit = self.taken.iter().any(|t| t == name);
                Ok(if hit {
                    vec![format!("bob/{name}")]
                } else {
                    vec![]
                })
            }
        }
    }
    fn contains(&self, _: &str) -> Result<Listing, Error> {
        Ok(Listing {
            total: 0,
            names: vec![],
        })
    }
}

/// Records every `dotfiles project` call instead of running it.
#[derive(Default)]
pub struct Recorder {
    pub calls: RefCell<Vec<Vec<String>>>,
    pub fail: bool,
}

impl Scaffold for Recorder {
    fn setup(&self, kind: &str, name: &str, root: &std::path::Path, no_push: bool) -> Result<()> {
        let args = crate::delegate::setup_args(kind, name, root, no_push);
        self.calls.borrow_mut().push(args);
        if self.fail {
            anyhow::bail!("dotfiles project setup failed");
        }
        Ok(())
    }
}

/// A scripted platform: records every call, answers from a fixed pull request.
pub struct FakeForge {
    pub calls: RefCell<Vec<String>>,
    pr: PullRequest,
    /// Mergeability answered by the views after the first, in order; `Unknown` when exhausted.
    polls: RefCell<Vec<Mergeable>>,
    views: Cell<usize>,
    /// The operation that fails: `view`, `poll` (any view after the first), `merge`, `create`.
    fail: Option<&'static str>,
}

impl FakeForge {
    pub fn pr(title: &str, mergeable: Mergeable) -> Self {
        let pr = PullRequest {
            number: 8,
            title: title.into(),
            head: "feature".into(),
            base: "main".into(),
            author: "ana".into(),
            mergeable,
            review: "APPROVED".into(),
            merge_state: "CLEAN".into(),
        };
        Self {
            calls: RefCell::default(),
            pr,
            polls: RefCell::default(),
            views: Cell::new(0),
            fail: None,
        }
    }

    pub fn polling(self, later: &[Mergeable]) -> Self {
        *self.polls.borrow_mut() = later.iter().rev().cloned().collect();
        self
    }

    pub fn failing(mut self, operation: &'static str) -> Self {
        self.fail = Some(operation);
        self
    }

    pub fn log(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    fn check(&self, operation: &str) -> Result<(), ForgeError> {
        match self.fail {
            Some(f) if f == operation => Err(ForgeError::Run(format!("{operation} failed"))),
            _ => Ok(()),
        }
    }
}

impl Forge for FakeForge {
    fn view_pr(&self, number: Option<u64>) -> Result<PullRequest, ForgeError> {
        self.calls.borrow_mut().push(match number {
            Some(n) => format!("view {n}"),
            None => "view".into(),
        });
        let first = self.views.get() == 0;
        self.views.set(self.views.get() + 1);
        self.check(if first { "view" } else { "poll" })?;
        let mut pr = self.pr.clone();
        if !first {
            pr.mergeable = self.polls.borrow_mut().pop().unwrap_or(Mergeable::Unknown);
        }
        Ok(pr)
    }

    fn create_pr(&self, pr: &NewPullRequest) -> Result<String, ForgeError> {
        self.calls.borrow_mut().push(format!(
            "create --base {} --head {} --title {} --body {}",
            pr.base, pr.head, pr.title, pr.body
        ));
        self.check("create")?;
        Ok("https://github.com/o/r/pull/9".into())
    }

    fn merge_pr(&self, number: u64) -> Result<(), ForgeError> {
        self.calls.borrow_mut().push(format!("merge {number}"));
        self.check("merge")
    }
}
