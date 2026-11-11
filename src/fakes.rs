//! Test doubles for the two seams: GitHub (`RepoNames`) and `dotfiles` (`Scaffold`).
use crate::pipeline::Scaffold;
use actautoma::forge::{
    CiRun, Error as ForgeError, Forge, Mergeable, NewPullRequest, NewRepo, OpenPr, PullRequest,
    Repo,
};
use actit::contains::Listing;
use actit::error::Error;
use actit::ports::RepoNames;
use anyhow::Result;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
    pub calls: Rc<RefCell<Vec<String>>>,
    pr: PullRequest,
    repo: Repo,
    /// Mergeability answered by the views after the first, in order; `Unknown` when exhausted.
    polls: RefCell<Vec<Mergeable>>,
    views: Cell<usize>,
    open: Vec<OpenPr>,
    run: Option<CiRun>,
    /// The operation that fails: `view`, `poll` (any view after the first), `merge`, `create`,
    /// `view_repo`, `set_default_branch`, `create_repo`, `available`, `open_prs`, `last_run`.
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
        let repo = Repo {
            slug: "me/ss".into(),
            default_branch: "master".into(),
        };
        Self {
            calls: Rc::default(),
            pr,
            repo,
            polls: RefCell::default(),
            views: Cell::new(0),
            open: vec![],
            run: None,
            fail: None,
        }
    }

    /// The hosted repository `view_repo` answers with.
    pub fn hosting(mut self, slug: &str, default_branch: &str) -> Self {
        self.repo = Repo {
            slug: slug.into(),
            default_branch: default_branch.into(),
        };
        self
    }

    /// The open pull requests `open_prs` answers with.
    pub fn open(mut self, prs: Vec<OpenPr>) -> Self {
        self.open = prs;
        self
    }

    /// The CI run `last_run` answers with.
    pub fn running(mut self, run: CiRun) -> Self {
        self.run = Some(run);
        self
    }

    /// Records into `log` too, to see the order against another fake's calls.
    pub fn logging_to(mut self, log: Rc<RefCell<Vec<String>>>) -> Self {
        self.calls = log;
        self
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

    fn view_repo(&self) -> Result<Repo, ForgeError> {
        self.calls.borrow_mut().push("view_repo".into());
        self.check("view_repo")?;
        Ok(self.repo.clone())
    }

    fn set_default_branch(&self, slug: &str, branch: &str) -> Result<(), ForgeError> {
        let line = format!("set_default_branch {slug} {branch}");
        self.calls.borrow_mut().push(line);
        self.check("set_default_branch")
    }

    fn create_repo(&self, repo: &NewRepo) -> Result<(), ForgeError> {
        let vis = if repo.private { "private" } else { "public" };
        let push = if repo.push { " push" } else { "" };
        let line = format!(
            "create_repo {} {vis}{push} {}",
            repo.name,
            repo.source.display()
        );
        self.calls.borrow_mut().push(line);
        self.check("create_repo")
    }

    fn open_prs(&self) -> Result<Vec<OpenPr>, ForgeError> {
        self.calls.borrow_mut().push("open_prs".into());
        self.check("open_prs")?;
        Ok(self.open.clone())
    }

    fn last_run(&self, branch: &str) -> Result<Option<CiRun>, ForgeError> {
        self.calls.borrow_mut().push(format!("last_run {branch}"));
        self.check("last_run")?;
        Ok(self.run.clone())
    }

    fn available(&self) -> Result<(), ForgeError> {
        self.calls.borrow_mut().push("available".into());
        self.check("available")
    }
}

impl Default for FakeForge {
    fn default() -> Self {
        Self::pr("default", Mergeable::Mergeable)
    }
}
