//! The steps of a branch rename as data: printed by `--dry-run`, run by `rename`.
use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opts {
    pub from: String,
    pub to: String,
    /// Also delete the old branch on `origin`: irreversible, so never the default.
    pub delete_old: bool,
}

/// One step of the rename: a `git` command, or an operation on the hosting platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Git(Vec<String>),
    SetDefaultBranch { slug: String, branch: String },
}

impl Step {
    fn git(args: &[&str]) -> Self {
        Step::Git(args.iter().map(|a| a.to_string()).collect())
    }

    /// What the step does, as printed by `--dry-run` and in errors.
    pub fn line(&self) -> String {
        match self {
            Step::Git(args) => format!("git {}", args.join(" ")),
            Step::SetDefaultBranch { slug, branch } => {
                format!("set default branch of {slug} to {branch}")
            }
        }
    }
}

/// The local rename, then (when `origin` is a GitHub repository `slug`) the
/// push, the default-branch switch, the optional deletion and `origin/HEAD`.
/// The default is switched before the old branch goes: GitHub refuses to
/// delete the default branch.
pub fn plan(o: &Opts, slug: Option<&str>) -> Vec<Step> {
    let mut steps = vec![Step::git(&["branch", "-m", &o.from, &o.to])];
    let Some(slug) = slug else { return steps };
    steps.push(Step::git(&["push", "-u", "origin", &o.to]));
    steps.push(Step::SetDefaultBranch {
        slug: slug.to_string(),
        branch: o.to.clone(),
    });
    if o.delete_old {
        steps.push(Step::git(&["push", "origin", "--delete", &o.from]));
    }
    steps.push(Step::git(&["remote", "set-head", "origin", "-a"]));
    steps
}

/// The rules of `git check-ref-format`, plus: no leading `-`, so a name can
/// never be read as an option by the git or gh process it is handed to.
pub fn valid_branch(name: &str) -> Result<()> {
    let bad = |why: &str| bail!("invalid branch name {name:?}: {why}");
    if name.is_empty() || name == "@" {
        return bad("empty or reserved");
    }
    if name.starts_with('-') {
        return bad("starts with '-'");
    }
    if name.contains("..") || name.contains("@{") {
        return bad("contains '..' or '@{'");
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
    {
        return bad("contains a space, control or special character");
    }
    for part in name.split('/') {
        if part.is_empty()
            || part.starts_with('.')
            || part.ends_with(".lock")
            || part.ends_with('.')
        {
            return bad("an empty, dotted or '.lock' path component");
        }
    }
    Ok(())
}

pub fn validate(o: &Opts) -> Result<()> {
    valid_branch(&o.from)?;
    valid_branch(&o.to)?;
    if o.from == o.to {
        bail!("{:?} and {:?} are the same branch name", o.from, o.to);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("plan_tests.rs");
}
