//! Test doubles for the two seams: GitHub (`RepoNames`) and `dotfiles` (`Scaffold`).
use crate::pipeline::Scaffold;
use actit::contains::Listing;
use actit::error::Error;
use actit::ports::RepoNames;
use anyhow::Result;
use std::cell::RefCell;

pub struct Registry {
    taken: Vec<String>,
    failure: Option<Error>,
}

impl Registry {
    pub fn free() -> Self {
        Self {
            taken: vec![],
            failure: None,
        }
    }

    pub fn taking(names: &[&str]) -> Self {
        Self {
            taken: names.iter().map(|s| s.to_string()).collect(),
            failure: None,
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            taken: vec![],
            failure: Some(Error::Github(message.into())),
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
        match &self.failure {
            Some(e) => Err(e.clone()),
            None if self.taken.iter().any(|t| t == name) => Ok(vec![format!("bob/{name}")]),
            None => Ok(vec![]),
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
    fn setup(&self, args: &[String]) -> Result<()> {
        self.calls.borrow_mut().push(args.to_vec());
        if self.fail {
            anyhow::bail!("dotfiles project setup failed");
        }
        Ok(())
    }
}
