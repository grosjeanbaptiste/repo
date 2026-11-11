//! The outside world of a deploy: programs to run and to look for on `PATH`.
use anyhow::Result;
use std::path::Path;

/// One program invocation; `env` is added to the inherited environment.
pub struct Call<'a> {
    pub prog: &'a str,
    pub args: &'a [String],
    pub cwd: Option<&'a Path>,
    pub env: &'a [(String, String)],
}

pub trait Tool {
    /// Runs with the terminal's stdio; a non-zero exit is an `Err` naming the program and its status.
    fn run(&self, call: &Call) -> Result<()>;
    /// Stdout, trimmed, of a program that exited 0; `None` when it exited non-zero. `Err` only when it cannot start.
    fn capture(&self, call: &Call) -> Result<Option<String>>;
    fn on_path(&self, prog: &str) -> bool;
}

/// A call with no directory and no extra environment.
pub fn plain<'a>(prog: &'a str, args: &'a [String]) -> Call<'a> {
    Call {
        prog,
        args,
        cwd: None,
        env: &[],
    }
}
