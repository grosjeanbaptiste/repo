//! Scripted `Tool` for the unit tests: records every call, runs nothing.
use super::tool::{Call, Tool};
use anyhow::{Result, bail};
use std::cell::RefCell;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    pub prog: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

#[derive(Default)]
pub struct Fake {
    pub runs: RefCell<Vec<Rec>>,
    /// Every `capture`, in order.
    pub captured: RefCell<Vec<Rec>>,
    /// Programs whose `run` fails.
    pub failing: Vec<String>,
    /// Programs that are not on `PATH`.
    pub missing: Vec<String>,
    /// `(prog, first argument)` -> captured stdout (`None` = non-zero exit); anything else captures `None`.
    pub captures: Vec<(String, String, Option<String>)>,
    /// `(prog, first argument, file)`: running it creates `file`, like a build leaving its output.
    pub on_run: Vec<(String, String, PathBuf)>,
}

impl Fake {
    pub fn runs(&self) -> Vec<Rec> {
        self.runs.borrow().clone()
    }

    /// `prog args...` of every run, for compact assertions.
    pub fn lines(&self) -> Vec<String> {
        self.runs()
            .iter()
            .map(|r| {
                format!("{} {}", r.prog, r.args.join(" "))
                    .trim()
                    .to_string()
            })
            .collect()
    }
}

fn rec(c: &Call) -> Rec {
    Rec {
        prog: c.prog.into(),
        args: c.args.to_vec(),
        cwd: c.cwd.map(Into::into),
        env: c.env.to_vec(),
    }
}

impl Tool for Fake {
    fn run(&self, call: &Call) -> Result<()> {
        self.runs.borrow_mut().push(rec(call));
        let first = call.args.first().map(String::as_str).unwrap_or("");
        for (_, _, file) in self
            .on_run
            .iter()
            .filter(|(p, a, _)| p == call.prog && a == first)
        {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(file, "built")?;
        }
        if self.failing.iter().any(|p| p == call.prog) {
            bail!("{} exited with status 1", call.prog);
        }
        Ok(())
    }

    fn capture(&self, call: &Call) -> Result<Option<String>> {
        self.captured.borrow_mut().push(rec(call));
        let first = call.args.first().map(String::as_str).unwrap_or("");
        Ok(self
            .captures
            .iter()
            .find(|(p, a, _)| p == call.prog && a == first)
            .and_then(|(.., out)| out.clone()))
    }

    fn on_path(&self, prog: &str) -> bool {
        !self.missing.iter().any(|p| p == prog)
    }
}

/// Runs a channel function with a throwaway home and returns its result and the recorded calls.
pub fn drive(
    env: super::target::Env,
    root: &std::path::Path,
    pairs: &[(&str, &str)],
    tool: Fake,
    f: impl FnOnce(&mut super::ctx::Ctx) -> Result<()>,
) -> (Result<()>, Fake) {
    let home = std::env::temp_dir().join("deploy-fake-home-unused");
    let vars = super::vars::Vars::of(pairs, &home);
    let (mut out, mut err) = (vec![], vec![]);
    let result = {
        let mut c = super::ctx::Ctx {
            env,
            root,
            vars: &vars,
            tool: &tool,
            out: &mut out,
            err: &mut err,
        };
        f(&mut c)
    };
    (result, tool)
}

pub fn message(r: &Result<()>) -> String {
    r.as_ref().err().map(|e| e.to_string()).unwrap_or_default()
}
