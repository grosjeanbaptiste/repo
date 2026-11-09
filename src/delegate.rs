//! Everything `dotfiles project` already does is delegated to it.
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub fn setup_args(kind: &str, name: &str, root: &Path, no_push: bool) -> Vec<String> {
    let mut args = ["setup", kind, name, "--path"].map(String::from).to_vec();
    args.push(root.display().to_string());
    if no_push {
        args.push("--no-push".into());
    }
    args
}

pub fn project(args: &[String]) -> Result<()> {
    let status = Command::new("dotfiles")
        .arg("project")
        .args(args)
        .status()
        .context("dotfiles not found: repo needs it")?;
    if !status.success() {
        bail!("dotfiles project {} exited with {status}", args.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_args_pass_path_and_no_push() {
        let a = setup_args("package/rust", "actgit", Path::new("/s"), true);
        assert_eq!(
            a,
            [
                "setup",
                "package/rust",
                "actgit",
                "--path",
                "/s",
                "--no-push"
            ]
        );
    }

    #[test]
    fn setup_args_push_by_default() {
        assert!(!setup_args("doc", "x", Path::new("/s"), false).contains(&"--no-push".into()));
    }
}
