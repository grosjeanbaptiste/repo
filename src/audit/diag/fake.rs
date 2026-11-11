//! Scripted host for the check tests: tools on `PATH`, and answers for commands.
use super::Host;
use crate::scaffold::setup::Exec;
use anyhow::Result;
use std::cell::RefCell;
use std::path::Path;

pub struct Fake {
    pub tools: Vec<&'static str>,
    /// `(command line prefix, Ok(stdout) | Err(message))`, first match wins.
    pub answers: Vec<(
        &'static str,
        std::result::Result<&'static str, &'static str>,
    )>,
    pub calls: RefCell<Vec<String>>,
}

impl Fake {
    pub fn new(tools: &[&'static str]) -> Self {
        Self {
            tools: tools.to_vec(),
            answers: vec![],
            calls: RefCell::new(vec![]),
        }
    }

    pub fn answering(
        mut self,
        prefix: &'static str,
        r: std::result::Result<&'static str, &'static str>,
    ) -> Self {
        self.answers.push((prefix, r));
        self
    }

    /// Like `answering`, but wins over the answers already given.
    pub fn overriding(
        mut self,
        prefix: &'static str,
        r: std::result::Result<&'static str, &'static str>,
    ) -> Self {
        self.answers.insert(0, (prefix, r));
        self
    }

    pub fn host<'a>(&'a self, has: &'a dyn Fn(&str) -> bool) -> Host<'a> {
        Host {
            has_tool: has,
            exec: self,
        }
    }

    pub fn has(&self) -> impl Fn(&str) -> bool + '_ {
        |t| self.tools.contains(&t)
    }
}

impl Exec for Fake {
    fn run(&self, prog: &str, args: &[&str], _: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        match self.answers.iter().find(|(p, _)| line.starts_with(p)) {
            Some((_, Ok(out))) => Ok((*out).to_string()),
            Some((_, Err(why))) => anyhow::bail!("{why}"),
            None => anyhow::bail!("unexpected command: {line}"),
        }
    }
}
