//! `repo workspace` end to end: temp workspaces, real git, a temp template tree. No network.
use std::fs;
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

struct Fx {
    _d: tempfile::TempDir,
    dir: PathBuf,
    templates: PathBuf,
}

fn fx(json: &str) -> Fx {
    let d = ok(tempfile::tempdir());
    let dir = ok(d.path().canonicalize());
    let ws = dir.join("ws");
    ok(fs::create_dir(&ws));
    ok(fs::write(ws.join("w.code-workspace"), json));
    let t = dir.join("tpl/project-templates");
    ok(fs::create_dir_all(t.join("package/rust")));
    ok(fs::write(
        t.join("package/rust/Cargo.toml.tmpl"),
        "name = \"{{project_name}}\"\n",
    ));
    Fx {
        dir: ws,
        templates: dir.join("tpl"),
        _d: d,
    }
}

impl Fx {
    fn repo(&self, args: &[&str]) -> Output {
        self.repo_in(&self.dir, args)
    }
    fn repo_in(&self, cwd: &Path, args: &[&str]) -> Output {
        ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("workspace")
            .args(args)
            .current_dir(cwd)
            .env("PROJECT_REPO_ROOT", &self.templates)
            .env_remove("DOTFILES_WORKSPACE_FILE")
            .output())
    }
    fn json(&self) -> String {
        ok(fs::read_to_string(self.dir.join("w.code-workspace")))
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn list_finds_the_workspace_from_a_subdirectory() {
    let f = fx(r#"{"folders":[{"name":"api","path":"a"}]}"#);
    ok(fs::create_dir_all(f.dir.join("deep/er")));
    let o = f.repo_in(&f.dir.join("deep/er"), &["list"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).contains("api") && out(&o).starts_with("NAME"),
        "{}",
        out(&o)
    );
}

#[test]
fn no_workspace_fails_with_exit_1_and_the_hint() {
    let d = ok(tempfile::tempdir());
    let o = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(["workspace", "list"])
        .current_dir(d.path())
        .env_remove("DOTFILES_WORKSPACE_FILE")
        .output());
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("No *.code-workspace file in")
            && err(&o).contains("DOTFILES_WORKSPACE_FILE=<path>"),
        "{}",
        err(&o)
    );
}

#[test]
fn the_env_variable_names_the_workspace() {
    let f = fx(r#"{"folders":[{"path":"x"}]}"#);
    let elsewhere = ok(tempfile::tempdir());
    let o = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(["workspace", "list"])
        .current_dir(elsewhere.path())
        .env("DOTFILES_WORKSPACE_FILE", f.dir.join("w.code-workspace"))
        .output());
    assert!(o.status.success(), "{}", err(&o));
    assert!(out(&o).contains(" x\n"), "{}", out(&o));
}

#[test]
fn add_adopts_a_directory_then_remove_forgets_it() {
    let f = fx("{}");
    ok(fs::create_dir(f.dir.join("api")));
    let o = f.repo(&["add", "api"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(f.json().contains("\"path\": \"api\""), "{}", f.json());
    let o = f.repo(&["remove", "api"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(!f.json().contains("api"), "{}", f.json());
    assert!(f.dir.join("api").is_dir());
}

#[test]
fn add_of_a_missing_directory_fails_and_remove_of_an_unlisted_one_too() {
    let f = fx("{}");
    assert_eq!(f.repo(&["add", "nope"]).status.code(), Some(1));
    let o = f.repo(&["remove", "nope"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("nope not in w.code-workspace; nothing to remove."),
        "{}",
        err(&o)
    );
}

#[test]
fn add_clone_and_init_are_mutually_exclusive() {
    let f = fx("{}");
    let o = f.repo(&["add", "x", "--clone", "u", "--init", "package/rust"]);
    assert_eq!(o.status.code(), Some(2), "{}", err(&o));
}

#[test]
fn add_clone_clones_and_lists() {
    let f = fx("{}");
    let up = f.dir.join("upstream");
    ok(fs::create_dir(&up));
    git(&up, &["init", "-q", "-b", "main"]);
    git(&up, &["commit", "-q", "--allow-empty", "-m", "x"]);
    let o = f.repo(&["add", "lib", "--clone", &up.display().to_string()]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(f.dir.join("lib/.git").exists());
    assert!(f.json().contains("\"path\": \"lib\""));
}

#[test]
fn add_init_scaffolds_from_the_templates_and_lists() {
    let f = fx("{}");
    let o = f.repo(&["add", "svc", "--init", "package/rust"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(
        ok(fs::read_to_string(f.dir.join("svc/Cargo.toml"))),
        "name = \"svc\"\n"
    );
    assert!(f.json().contains("\"path\": \"svc\""));
}

#[test]
fn add_init_of_an_unknown_type_fails_and_changes_nothing() {
    let f = fx("{}");
    let o = f.repo(&["add", "svc", "--init", "nope/nope"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(f.json(), "{}");
}

#[test]
fn status_and_push_dry_run_see_a_repo_ahead() {
    let f = fx(r#"{"folders":[{"path":"api"}]}"#);
    let remote = f.dir.join("api.git");
    ok(fs::create_dir_all(&remote));
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let work = f.dir.join("api");
    ok(fs::create_dir_all(&work));
    git(&work, &["init", "-q", "-b", "main"]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(
        &work,
        &["remote", "add", "origin", &remote.display().to_string()],
    );
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "two"]);
    let o = f.repo(&["status"]);
    assert!(out(&o).contains("ahead:1"), "{}", out(&o));
    let o = f.repo(&["push", "-n"]);
    assert!(
        out(&o).contains("would push api") && out(&o).contains("dry-run: 1 would push"),
        "{}",
        out(&o)
    );
    let o = f.repo(&["push"]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).contains("1 pushed, 0 up-to-date/skipped"),
        "{}",
        out(&o)
    );
    assert!(out(&f.repo(&["status"])).contains("up-to-date"));
}

#[test]
fn a_broken_workspace_fails_loudly_on_every_subcommand() {
    let f = fx("// not json");
    for args in [
        vec!["list"],
        vec!["status"],
        vec!["push"],
        vec!["remove", "a"],
    ] {
        let o = f.repo(&args);
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        assert!(err(&o).contains("not valid JSON"), "{args:?}: {}", err(&o));
    }
}

#[test]
fn a_missing_git_fails_loudly_for_status_and_push() {
    let f = fx("{}");
    let empty = f.dir.join("emptybin");
    ok(fs::create_dir(&empty));
    for sub in ["status", "push"] {
        let o = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .args(["workspace", sub])
            .current_dir(&f.dir)
            .env("PATH", &empty)
            .output());
        assert_eq!(o.status.code(), Some(1), "{sub}");
        assert!(err(&o).contains("git not found"), "{sub}: {}", err(&o));
    }
}
