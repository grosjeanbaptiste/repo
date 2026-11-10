//! `repo audit` end to end on temp projects, a temp secret store and a controlled environment.
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

struct Fx {
    _d: tempfile::TempDir,
    project: std::path::PathBuf,
    state: std::path::PathBuf,
    empty_bin: std::path::PathBuf,
}

fn fx(env_example: &str) -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let (project, state, empty_bin) = (base.join("p"), base.join("state"), base.join("bin"));
    for p in [&project, &state.join("secrets"), &empty_bin] {
        ok(fs::create_dir_all(p));
    }
    ok(fs::write(project.join(".env.example"), env_example));
    Fx {
        _d: d,
        project,
        state,
        empty_bin,
    }
}

impl Fx {
    fn audit(&self, arg: &Path, env: &[(&str, &str)]) -> Output {
        ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("audit")
            .arg(arg)
            .env_clear()
            .env("PATH", &self.empty_bin)
            .env("HOME", &self.state)
            .env("DOTFILES_STATE_ROOT", &self.state)
            .envs(env.iter().copied())
            .output())
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn a_missing_variable_fails_with_exit_1_after_the_report() {
    let f = fx("SET_VAR=1\nNPM_TOKEN=\nMISSING_VAR=\n");
    ok(fs::write(f.state.join("secrets/npmrc.age"), "x"));
    let o = f.audit(&f.project, &[("SET_VAR", "yes")]);
    assert_eq!(o.status.code(), Some(1), "{}", err(&o));
    let t = out(&o);
    assert!(t.contains("in env"), "{t}");
    assert!(t.contains("(enrolled as npmrc)"), "{t}");
    assert!(
        t.contains("MISSING_VAR")
            && t.contains("1 in env, 1 in dotfiles (not exported), 1 missing"),
        "{t}"
    );
    assert!(
        err(&o).contains("audit: 1 variable(s) missing"),
        "{}",
        err(&o)
    );
}

#[test]
fn everything_fulfilled_exits_0() {
    let f = fx("SET_VAR=1\n");
    let o = f.audit(&f.project, &[("SET_VAR", "")]);
    assert!(o.status.success(), "{}{}", out(&o), err(&o));
    assert!(out(&o).contains("1 in env, 0 in dotfiles (not exported), 0 missing"));
}

#[test]
fn an_empty_but_set_variable_counts_as_in_env() {
    let f = fx("EMPTY_ONE=\n");
    assert!(f.audit(&f.project, &[("EMPTY_ONE", "")]).status.success());
}

#[test]
fn findings_come_from_the_tools_on_path() {
    let f = fx("");
    ok(fs::write(f.project.join("mise.toml"), ""));
    let o = f.audit(&f.project, &[]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).contains("mise.toml present but mise not installed"),
        "{}",
        out(&o)
    );
}

#[test]
fn a_tool_on_path_turns_the_warning_into_ok() {
    let f = fx("");
    ok(fs::write(f.project.join("mise.toml"), ""));
    let mise = f.empty_bin.join("mise");
    ok(fs::write(&mise, "#!/bin/sh\n"));
    ok(Command::new("chmod").arg("+x").arg(&mise).status());
    assert!(
        out(&f.audit(&f.project, &[])).contains("mise.toml present — `mise install` to provision")
    );
}

#[test]
fn a_path_that_is_not_a_directory_fails_loudly() {
    let f = fx("");
    let o = f.audit(&f.project.join("nope"), &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("audit: path is not a directory:"),
        "{}",
        err(&o)
    );
}

#[test]
fn the_default_path_is_the_current_directory() {
    let f = fx("ONLY_HERE=\n");
    let o = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .arg("audit")
        .current_dir(&f.project)
        .env_clear()
        .env("PATH", &f.empty_bin)
        .env("DOTFILES_STATE_ROOT", &f.state)
        .output());
    assert_eq!(o.status.code(), Some(1));
    assert!(
        out(&o).contains("ONLY_HERE")
            && out(&o).contains(&format!("== auditing {} ==", f.project.display())),
        "{}",
        out(&o)
    );
}
