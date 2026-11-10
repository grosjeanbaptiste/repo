//! The real `Exec` and the `Scaffold` that uses it: no `dotfiles` involved.
use super::setup::{Exec, Setup, setup};
use crate::pipeline::Scaffold;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Process;

impl Exec for Process {
    fn run(&self, prog: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
        let mut cmd = Command::new(prog);
        cmd.args(args).stderr(Stdio::inherit());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd
            .output()
            .with_context(|| format!("{prog} not found: repo needs it"))?;
        if !out.status.success() {
            bail!("{prog} {} exited with {}", args.join(" "), out.status);
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Like `Process`, but stderr is captured into the error: for probes whose
/// failure is an expected answer (no remote, not a repository), not noise.
pub struct Quiet;

impl Exec for Quiet {
    fn run(&self, prog: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
        let mut cmd = Command::new(prog);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd
            .output()
            .with_context(|| format!("{prog} not found: repo needs it"))?;
        if !out.status.success() {
            let why = String::from_utf8_lossy(&out.stderr);
            bail!(
                "{prog} {} exited with {}: {}",
                args.join(" "),
                out.status,
                why.trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Proleptic Gregorian year of a Unix timestamp (days-from-civil, H. Hinnant).
pub fn year_of(secs: u64) -> u64 {
    let z = secs / 86_400 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    yoe + era * 400 + u64::from(mp >= 10)
}

/// Templates live in dotfiles (`PROJECT_REPO_ROOT`/`DOTFILES_REPO_ROOT`, else `~/dotfiles`).
pub fn templates_root(env: impl Fn(&str) -> Option<String>) -> Result<PathBuf> {
    for key in ["PROJECT_REPO_ROOT", "DOTFILES_REPO_ROOT"] {
        if let Some(v) = env(key).filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(v).join("project-templates"));
        }
    }
    let home = env("HOME")
        .filter(|h| !h.is_empty())
        .context("HOME is not set")?;
    Ok(PathBuf::from(home).join("dotfiles/project-templates"))
}

pub fn year_now() -> Result<String> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before 1970")?;
    Ok(year_of(secs.as_secs()).to_string())
}

pub struct Native {
    pub templates: PathBuf,
}

impl Native {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            templates: templates_root(|k| std::env::var(k).ok())?,
        })
    }
}

impl Scaffold for Native {
    fn setup(&self, kind: &str, name: &str, root: &Path, no_push: bool) -> Result<()> {
        let s = Setup {
            templates: &self.templates,
            parent: root,
            kind,
            name,
            no_push,
            year: year_now()?,
        };
        let target = setup(&s, &Process)?;
        if no_push {
            println!("Local scaffold ready at {}", target.display());
            println!(
                "Push manually when ready:\n  gh repo create {name} --private --source {} --push",
                target.display()
            );
        } else {
            println!("Ready. cd {} to start.", target.display());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quiet_captures_stderr_into_the_error_instead_of_printing_it() {
        let e = Quiet
            .run("sh", &["-c", "echo boom >&2; exit 4"], None)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(e.contains("exited with") && e.contains("boom"), "{e}");
    }

    #[test]
    fn quiet_returns_stdout_on_success_and_names_a_missing_program() {
        assert_eq!(ok(Quiet.run("echo", &["hi"], None)), "hi\n");
        let e = Quiet
            .run("zz-no-such-tool", &[], None)
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap_or_default();
        assert!(
            e.contains("zz-no-such-tool not found: repo needs it"),
            "{e}"
        );
    }

    #[test]
    fn quiet_runs_in_the_given_directory() {
        let d = ok(tempfile::tempdir());
        let want = ok(d.path().canonicalize());
        assert_eq!(
            ok(Quiet.run("pwd", &[], Some(d.path()))).trim(),
            want.to_string_lossy()
        );
    }

    fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
        r.unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn process_returns_stdout_of_a_successful_command() {
        assert_eq!(ok(Process.run("echo", &["hi"], None)), "hi\n");
    }

    #[test]
    fn process_runs_in_the_given_directory() {
        let d = ok(tempfile::tempdir());
        let out = ok(Process.run("pwd", &[], Some(d.path())));
        let want = ok(d.path().canonicalize());
        assert_eq!(out.trim(), want.to_string_lossy());
    }

    #[test]
    fn process_failure_names_the_command_and_status() {
        let e = Process
            .run("false", &[], None)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            e.starts_with("false  exited with") || e.contains("exited with"),
            "{e}"
        );
        let e = Process
            .run("sh", &["-c", "exit 3"], None)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(e.contains("sh -c exit 3") && e.contains('3'), "{e}");
    }

    #[test]
    fn process_missing_program_says_so() {
        let e = Process
            .run("zz-no-such-tool", &[], None)
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap_or_default();
        assert!(
            e.contains("zz-no-such-tool not found: repo needs it"),
            "{e}"
        );
    }

    fn templates() -> tempfile::TempDir {
        let d = ok(tempfile::tempdir());
        ok(std::fs::create_dir_all(d.path().join("common")));
        ok(std::fs::create_dir_all(d.path().join("package/rust")));
        ok(std::fs::write(
            d.path().join("common/README.md.tmpl"),
            "# {{project_name}} {{year}}\n",
        ));
        d
    }

    #[test]
    fn native_scaffolds_a_committed_repository_without_pushing() {
        let (t, root) = (templates(), ok(tempfile::tempdir()));
        let native = Native {
            templates: t.path().to_path_buf(),
        };
        ok(native.setup("package/rust", "demo", root.path(), true));
        let dir = root.path().join("demo");
        assert!(ok(std::fs::read_to_string(dir.join("README.md"))).starts_with("# demo 20"));
        let log = ok(Process.run("git", &["log", "--format=%s"], Some(&dir)));
        assert_eq!(log.trim(), "initial scaffold for demo (package/rust)");
    }

    #[test]
    fn native_refuses_an_unknown_type_and_an_existing_target() {
        let (t, root) = (templates(), ok(tempfile::tempdir()));
        let native = Native {
            templates: t.path().to_path_buf(),
        };
        let e = native
            .setup("package/cobol", "demo", root.path(), true)
            .err()
            .map(|e| e.to_string());
        assert_eq!(e.as_deref(), Some("Unknown project type: package/cobol"));
        ok(std::fs::create_dir(root.path().join("demo")));
        let e = native
            .setup("package/rust", "demo", root.path(), true)
            .err()
            .map(|e| e.to_string());
        assert!(e.is_some_and(|m| m.starts_with("Target already exists")));
    }

    use super::*;

    #[test]
    fn year_of_agrees_with_a_day_by_day_calendar_until_2500() {
        let leap = |y: u64| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let (mut year, mut day_in_year) = (1970_u64, 0_u64);
        for day in 0..(530 * 366) {
            assert_eq!(year_of(day * 86_400), year, "day {day}");
            assert_eq!(year_of(day * 86_400 + 86_399), year, "end of day {day}");
            day_in_year += 1;
            if day_in_year == if leap(year) { 366 } else { 365 } {
                year += 1;
                day_in_year = 0;
            }
        }
    }

    #[test]
    fn year_now_is_a_plausible_four_digit_year() {
        let y: u64 = year_now().ok().and_then(|y| y.parse().ok()).unwrap_or(0);
        assert!((2026..2200).contains(&y), "{y}");
    }

    #[test]
    fn year_of_known_instants() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(951_782_400), 2000); // 2000-02-29
        assert_eq!(year_of(1_767_225_599), 2025); // 2025-12-31T23:59:59Z
        assert_eq!(year_of(1_767_225_600), 2026); // 2026-01-01T00:00:00Z
        assert_eq!(year_of(4_107_542_400), 2100); // 2100-03-01, not a leap year
    }

    #[test]
    fn templates_root_prefers_overrides_then_home() {
        let env = |pairs: &'static [(&str, &str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let r = templates_root(env(&[
            ("PROJECT_REPO_ROOT", "/p"),
            ("DOTFILES_REPO_ROOT", "/d"),
        ]));
        assert_eq!(r.ok(), Some(PathBuf::from("/p/project-templates")));
        let r = templates_root(env(&[("DOTFILES_REPO_ROOT", "/d"), ("HOME", "/h")]));
        assert_eq!(r.ok(), Some(PathBuf::from("/d/project-templates")));
        let r = templates_root(env(&[("HOME", "/h")]));
        assert_eq!(r.ok(), Some(PathBuf::from("/h/dotfiles/project-templates")));
        let r = templates_root(env(&[]));
        assert_eq!(
            r.err().map(|e| e.to_string()).as_deref(),
            Some("HOME is not set")
        );
    }
}
