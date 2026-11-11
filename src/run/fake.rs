//! Scripted `Runner` for the unit tests of this module.
use super::exec::Runner;
use anyhow::Result;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

pub type Call = (String, Vec<String>, Option<PathBuf>);

pub struct Fake {
    pub calls: RefCell<Vec<Call>>,
    pub code: i32,
    pub spawn_error: bool,
}

impl Fake {
    pub fn exiting(code: i32) -> Self {
        Fake {
            calls: RefCell::new(vec![]),
            code,
            spawn_error: false,
        }
    }
}

impl Runner for Fake {
    fn me(&self) -> String {
        "repo-bin".into()
    }

    fn run(&self, prog: &str, args: &[String], cwd: Option<&Path>) -> Result<i32> {
        if self.spawn_error {
            anyhow::bail!("{prog}: not found");
        }
        self.calls
            .borrow_mut()
            .push((prog.into(), args.to_vec(), cwd.map(Path::to_path_buf)));
        Ok(self.code)
    }
}
