//! `repo run` end to end: a real registry and project directory, scripted `make`, `cargo`, `melos` and `dotfiles`.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Logs `<name> <cwd-basename> <args>` to $STAGE/calls.log, prints its own output, exits with $<NAME>_EXIT.
const TOOL: &str = r#"#!/bin/sh
name=$(basename "$0")
echo "$name|$(basename "$PWD")|$*" >> "$STAGE/calls.log"
echo "$name ran"
eval "code=\${$(echo "$name" | tr a-z A-Z)_EXIT:-0}"
exit "$code"
"#;

struct Fx {
    _d: tempfile::TempDir,
    base: PathBuf,
    project: PathBuf,
}

fn script(path: &Path, text: &str) {
    ok(fs::write(path, text));
    ok(fs::set_permissions(path, fs::Permissions::from_mode(0o755)));
}

fn fx(files: &[(&str, &str)]) -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let project = base.join("app");
    for p in [&project, &base.join("state"), &base.join("bin")] {
        ok(fs::create_dir_all(p));
    }
    for (rel, text) in files {
        let p = project.join(rel);
        ok(fs::create_dir_all(ok(p.parent().ok_or("no parent"))));
        ok(fs::write(p, text));
    }
    ok(fs::write(
        base.join("state/projects.txt"),
        format!("app|{}\n", project.display()),
    ));
    for t in ["make", "cargo", "melos", "flutter", "dotfiles"] {
        script(&base.join("bin").join(t), TOOL);
    }
    Fx {
        _d: d,
        base,
        project,
    }
}

impl Fx {
    fn repo(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        let path = format!("{}:/usr/bin:/bin", self.base.join("bin").display());
        ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("run")
            .args(args)
            .current_dir(&self.base)
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.base.join("state"))
            .env("DOTFILES_STATE_ROOT", self.base.join("state"))
            .env("STAGE", &self.base)
            .envs(extra.iter().copied())
            .output())
    }

    /// Where `repo` was started: tools without a project directory inherit it.
    fn cwd_name(&self) -> String {
        self.base
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.base.join("calls.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn a_makefile_target_by_registered_name_runs_make_and_shows_its_output() {
    let f = fx(&[("Makefile", "build:\n")]);
    let o = f.repo(&["app", "build"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(out(&o), "make ran\n");
    assert_eq!(
        f.calls(),
        vec![format!(
            "make|{}|-C {} build",
            ok(f.base.file_name().ok_or("n")).to_string_lossy(),
            f.project.display()
        )]
    );
}

#[test]
fn the_exit_code_of_the_project_command_is_the_exit_code_of_repo() {
    let f = fx(&[("Makefile", "build:\n")]);
    let o = f.repo(&["app", "build"], &[("MAKE_EXIT", "66")]);
    assert_eq!(o.status.code(), Some(66));
    assert!(err(&o).is_empty(), "no extra diagnostics: {}", err(&o));
}

#[test]
fn cargo_runs_in_the_project_directory_with_its_arguments() {
    let f = fx(&[("Cargo.toml", "")]);
    let o = f.repo(
        &[f.project.to_str().unwrap_or_default(), "test", "--", "-q"],
        &[],
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(f.calls(), vec!["cargo|app|test -- -q"]);
}

#[test]
fn no_verb_lists_the_verbs() {
    let f = fx(&[
        ("Makefile", "a:\nb:\n"),
        ("melos.yaml", "scripts:\n  gen:\n"),
    ]);
    let o = f.repo(&["app"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(out(&o), "a\nb\ngen\n");
    assert!(f.calls().is_empty());
}

#[test]
fn an_unknown_verb_fails_with_one_and_lists_the_available_ones() {
    let f = fx(&[("Makefile", "a:\n")]);
    let o = f.repo(&["app", "zz"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        err(&o),
        "'zz' isn't a known command for app.\nAvailable:\na\n"
    );
    assert!(f.calls().is_empty());
}

#[test]
fn an_unknown_project_is_named() {
    let f = fx(&[]);
    let o = f.repo(&["nope"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("Unknown project: nope"), "{}", err(&o));
}

#[test]
fn publish_goes_to_dotfiles_and_help_flags_reach_the_project_command() {
    let f = fx(&[("Cargo.toml", "")]);
    let o = f.repo(&["app", "publish", "test", "ios"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        f.calls(),
        vec![format!(
            "dotfiles|{}|project {} publish test ios",
            f.cwd_name(),
            f.project.display()
        )]
    );
    let o = f.repo(&["app", "build", "--help"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(
        f.calls()
            .last()
            .is_some_and(|c| c.ends_with("build --help")),
        "{:?}",
        f.calls()
    );
}

#[test]
fn a_missing_tool_is_a_loud_error() {
    let f = fx(&[("Cargo.toml", "")]);
    ok(fs::remove_file(f.base.join("bin/cargo")));
    let o = f.repo(&["app", "build"], &[]);
    assert_ne!(o.status.code(), Some(0));
    assert!(err(&o).contains("cannot start cargo"), "{}", err(&o));
}
