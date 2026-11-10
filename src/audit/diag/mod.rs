//! Tool and repository checks of `audit`: each says OK, WARN or FAIL, or nothing when its marker file is absent.
#[cfg(test)]
pub mod fake;
pub mod git;
pub mod tools;

use crate::scaffold::setup::Exec;
use anyhow::Result;
use std::fmt;
use std::path::Path;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Self::Ok => "OK",
            Self::Warn => "WARN",
            Self::Fail => "FAIL",
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub msg: String,
}

pub fn ok(msg: impl Into<String>) -> Finding {
    Finding {
        level: Level::Ok,
        msg: msg.into(),
    }
}

pub fn warn(msg: impl Into<String>) -> Finding {
    Finding {
        level: Level::Warn,
        msg: msg.into(),
    }
}

pub fn fail(msg: impl Into<String>) -> Finding {
    Finding {
        level: Level::Fail,
        msg: msg.into(),
    }
}

/// What the checks may ask of the machine: is a tool on `PATH`, and run a quiet command.
pub struct Host<'a> {
    pub has_tool: &'a dyn Fn(&str) -> bool,
    pub exec: &'a dyn Exec,
}

impl Host<'_> {
    pub fn run(&self, prog: &str, args: &[&str], cwd: &Path) -> Result<String> {
        self.exec.run(prog, args, Some(cwd))
    }
}

/// All checks, in the order of the bash version.
pub fn run_all(root: &Path, host: &Host) -> Result<Vec<Finding>> {
    let mut found = Vec::new();
    found.extend(git::submodules(root, host));
    found.extend(git::lfs(root, host)?);
    found.extend(git::precommit(root, host));
    found.extend(git::husky(root)?);
    found.extend(tools::tool_versions(root, host)?);
    found.extend(tools::nvmrc(root, host)?);
    found.extend(tools::python_version(root, host)?);
    found.extend(tools::mise_toml(root, host));
    found.extend(tools::package_managers(root, host));
    found.extend(tools::direnv(root, host));
    Ok(found)
}
