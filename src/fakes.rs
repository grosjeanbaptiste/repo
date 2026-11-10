//! Test doubles for the two seams: GitHub (`RepoNames`) and `dotfiles` (`Scaffold`).
use crate::pipeline::Scaffold;
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
