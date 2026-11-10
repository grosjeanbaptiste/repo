//! What `workspace status|push` says about one folder.
use crate::scaffold::setup::Exec;
use std::fmt;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    NotGit,
    Detached,
    NoRemote,
    NoUpstream,
    UpToDate,
    Ahead(u64),
    /// A git probe failed for a reason that is not one of the answers above.
    Error(String),
}

/// The words of the bash version (`ahead:N`, `up-to-date`, `no-upstream`, …).
impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("missing"),
            Self::NotGit => f.write_str("not-git"),
            Self::Detached => f.write_str("detached"),
            Self::NoRemote => f.write_str("no-remote"),
            Self::NoUpstream => f.write_str("no-upstream"),
            Self::UpToDate => f.write_str("up-to-date"),
            Self::Ahead(n) => write!(f, "ahead:{n}"),
            Self::Error(why) => write!(f, "error: {why}"),
        }
    }
}

/// `probe` must be quiet: a failing git command is an answer here, not noise.
pub fn classify(probe: &impl Exec, dir: &Path) -> State {
    if !dir.is_dir() {
        return State::Missing;
    }
    let git = |args: &[&str]| probe.run("git", args, Some(dir));
    if git(&["rev-parse", "--git-dir"]).is_err() {
        return State::NotGit;
    }
    // a detached HEAD makes `symbolic-ref` fail: that is its answer
    let Ok(head) = git(&["symbolic-ref", "--quiet", "--short", "HEAD"]) else {
        return State::Detached;
    };
    match git(&["remote"]) {
        Err(e) => return State::Error(e.to_string()),
        Ok(remotes) if remotes.trim().is_empty() => return State::NoRemote,
        Ok(_) => {}
    }
    // no tracking branch makes this fail: that is its answer
    let Ok(upstream) = git(&[
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        "@{upstream}",
    ]) else {
        return State::NoUpstream;
    };
    let range = format!("{}..{}", upstream.trim(), head.trim());
    match git(&["rev-list", "--count", &range]) {
        Err(e) => State::Error(e.to_string()),
        Ok(n) => match n.trim().parse::<u64>() {
            Ok(0) => State::UpToDate,
            Ok(n) => State::Ahead(n),
            Err(_) => State::Error(format!("git rev-list --count answered {:?}", n.trim())),
        },
    }
}

#[cfg(test)]
mod tests {
    include!("status_tests.rs");
}
