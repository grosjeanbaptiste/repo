//! `repo install` end to end: a real registry and git repository, scripted `dotfiles` and `cargo`.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) {
    let o = ok(Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .output());
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

const DOTFILES: &str = r#"#!/bin/sh
echo "$*" >> "$STAGE/heavy.log"
[ "$1" = heavy ] && [ "$2" = run ] && [ "$3" = -- ] || exit 97
shift 3
exec "$@"
"#;

const CARGO: &str = r#"#!/bin/sh
if [ "$1" = metadata ]; then
  printf '{"packages":[{"name":"ss","manifest_path":"%s/Cargo.toml"},{"name":"ss-ui","manifest_path":"%s/ui/Cargo.toml"}]}\n' "$PROJECT" "$PROJECT"
  exit 0
fi
[ "$1" = --version ] && { echo "cargo 1"; exit 0; }
echo "$*" >> "$STAGE/cargo.log"
echo "Compiling ss"
[ -z "$CARGO_FAIL" ] || exit 101
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

fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let project = base.join("ss");
    for p in [&project, &base.join("state"), &base.join("bin")] {
        ok(fs::create_dir_all(p));
    }
    ok(fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"ss\"\n",
    ));
    ok(fs::write(project.join("Cargo.lock"), ""));
    git(&project, &["init", "-q", "-b", "main"]);
    git(&project, &["add", "-A"]);
    git(&project, &["commit", "-q", "-m", "fixture"]);
    ok(fs::write(
        base.join("state/projects.txt"),
        format!("ss|{}\n", project.display()),
    ));
    script(&base.join("bin/dotfiles"), DOTFILES);
    script(&base.join("bin/cargo"), CARGO);
    Fx {
        _d: d,
        base,
        project,
    }
}

impl Fx {
    fn install(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        let path = format!("{}:/usr/bin:/bin", self.base.join("bin").display());
        ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("install")
            .args(args)
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.base.join("state"))
            .env("DOTFILES_STATE_ROOT", self.base.join("state"))
            .env("STAGE", &self.base)
            .env("PROJECT", &self.project)
            .envs(extra.iter().copied())
            .output())
    }

    fn log(&self, name: &str) -> String {
        fs::read_to_string(self.base.join(name))
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn the_binary_is_installed_through_the_heavy_queue_and_cargo_output_is_shown() {
    let f = fx();
    let o = f.install(&["ss", "--bin", "ss"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(out(&o).contains("Compiling ss"), "{}", out(&o));
    assert!(
        out(&o).contains(&format!("Installed ss from {}", f.project.display())),
        "{}",
        out(&o)
    );
    let want = format!("install --path {} --locked --bin ss", f.project.display());
    assert_eq!(f.log("cargo.log"), want);
    assert_eq!(f.log("heavy.log"), format!("heavy run -- cargo {want}"));
}

#[test]
fn a_dirty_tree_is_refused_before_compiling() {
    let f = fx();
    ok(fs::write(f.project.join("new.txt"), "x"));
    let o = f.install(&["ss", "--bin", "ss"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("has uncommitted changes"), "{}", err(&o));
    assert_eq!(f.log("heavy.log"), "");
}

#[test]
fn an_unknown_project_is_named() {
    let f = fx();
    let o = f.install(&["missing"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("Unknown project: missing"), "{}", err(&o));
}

#[test]
fn a_workspace_member_is_installed_from_its_directory() {
    let f = fx();
    let o = f.install(
        &["ss", "--package", "ss-ui", "--bin", "ss-ui", "--force"],
        &[],
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        f.log("cargo.log"),
        format!(
            "install --path {}/ui --locked --bin ss-ui --force",
            f.project.display()
        )
    );
    assert!(out(&o).contains("Installed ss-ui from "), "{}", out(&o));
}

#[test]
fn an_unknown_package_fails_before_compiling() {
    let f = fx();
    let o = f.install(&["ss", "--package", "nope"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("no package nope in "), "{}", err(&o));
    assert_eq!(f.log("heavy.log"), "");
}

#[test]
fn a_failing_install_exits_1_and_confirms_nothing() {
    let f = fx();
    let o = f.install(&["ss"], &[("CARGO_FAIL", "1")]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("Cargo installation failed for ss"),
        "{}",
        err(&o)
    );
    assert!(!out(&o).contains("Installed"), "{}", out(&o));
}

#[test]
fn a_missing_value_after_bin_is_a_usage_error() {
    let f = fx();
    let o = f.install(&["ss", "--bin"], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(f.log("heavy.log"), "");
}

#[test]
fn without_a_name_it_is_a_usage_error() {
    let f = fx();
    assert_eq!(f.install(&[], &[]).status.code(), Some(2));
}
