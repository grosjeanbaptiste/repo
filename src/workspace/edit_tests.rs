use super::*;
use crate::scaffold::native::Process;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

struct Ws {
    _d: tempfile::TempDir,
    dir: PathBuf,
    file: PathBuf,
}

fn ws(json: &str) -> Ws {
    let d = ok(tempfile::tempdir());
    let dir = ok(d.path().canonicalize());
    let file = dir.join("w.code-workspace");
    ok(fs::write(&file, json));
    Ws { _d: d, dir, file }
}

fn no_scaffold(_: &str, _: &Path) -> Result<()> {
    anyhow::bail!("scaffold must not run")
}

fn adopt(w: &Ws, name: &str) -> (Result<()>, String) {
    let mut out = Vec::new();
    let r = add(&w.file, name, &Source::Adopt, &Process, &no_scaffold, &mut out);
    (r, String::from_utf8_lossy(&out).into_owned())
}

fn paths(w: &Ws) -> Vec<String> {
    ok(super::super::file::folders(&ok(fs::read_to_string(&w.file)))).into_iter().map(|f| f.path).collect()
}

fn why<T>(r: Result<T>) -> String {
    r.err().map(|e| e.to_string()).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn adopting_an_existing_directory_adds_it() {
    let w = ws("{}");
    ok(fs::create_dir(w.dir.join("api")));
    let (r, out) = adopt(&w, "api");
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(out, "==> Added folder { name: api, path: api } to w.code-workspace\n");
    assert_eq!(paths(&w), ["api"]);
}

#[test]
fn adopting_a_missing_directory_is_refused_and_the_file_untouched() {
    let w = ws("{}");
    let (r, _) = adopt(&w, "api");
    let msg = why(r);
    assert!(msg.starts_with("workspace add api: ") && msg.contains("does not exist. Use --clone <url> or --init <type>, or create it first."), "{msg}");
    assert_eq!(ok(fs::read_to_string(&w.file)), "{}");
}

#[test]
fn adopting_a_listed_path_is_a_no_op() {
    let w = ws(r#"{"folders":[{"path":"api"}]}"#);
    ok(fs::create_dir(w.dir.join("api")));
    let before = ok(fs::read_to_string(&w.file));
    let (r, out) = adopt(&w, "api");
    assert!(r.is_ok());
    assert_eq!(out, "==> api already in w.code-workspace; no change.\n");
    assert_eq!(ok(fs::read_to_string(&w.file)), before);
}

#[test]
fn an_empty_name_is_refused() {
    let w = ws("{}");
    assert_eq!(why(adopt(&w, "").0), "add needs a name.");
}

#[test]
fn a_broken_workspace_fails_before_anything_is_touched() {
    let w = ws("not json");
    ok(fs::create_dir(w.dir.join("api")));
    assert!(adopt(&w, "api").0.is_err());
    assert_eq!(ok(fs::read_to_string(&w.file)), "not json");
}

#[test]
fn a_leftover_tmp_file_is_not_left_behind() {
    let w = ws("{}");
    ok(fs::create_dir(w.dir.join("api")));
    ok(adopt(&w, "api").0);
    let names: Vec<_> = ok(fs::read_dir(&w.dir)).map(|e| ok(e).file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

fn upstream_repo() -> (tempfile::TempDir, String) {
    let d = ok(tempfile::tempdir());
    let p = d.path();
    for args in [vec!["init", "-q", "-b", "main"], vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "x"]] {
        assert!(ok(Command::new("git").args(args).current_dir(p).status()).success());
    }
    let url = p.display().to_string();
    (d, url)
}

#[test]
fn cloning_puts_the_repo_in_the_workspace_and_lists_it() {
    let w = ws("{}");
    let (_u, url) = upstream_repo();
    let mut out = Vec::new();
    ok(add(&w.file, "lib", &Source::Clone(url.clone()), &Process, &no_scaffold, &mut out));
    let out = String::from_utf8_lossy(&out);
    assert!(out.starts_with(&format!("==> Cloning {url} into {}\n", w.dir.join("lib").display())), "{out}");
    assert!(out.ends_with("==> Added folder { name: lib, path: lib } to w.code-workspace\n"), "{out}");
    assert!(w.dir.join("lib/.git").exists());
    assert_eq!(paths(&w), ["lib"]);
}

#[test]
fn cloning_over_an_existing_path_is_refused() {
    let w = ws("{}");
    ok(fs::create_dir(w.dir.join("lib")));
    let r = add(&w.file, "lib", &Source::Clone("x".into()), &Process, &no_scaffold, &mut Vec::new());
    assert!(why(r).contains("already exists; refusing to clone over it"));
}

#[test]
fn a_failing_clone_fails_and_the_workspace_is_unchanged() {
    let w = ws("{}");
    let r = add(&w.file, "lib", &Source::Clone("/no/such/repo".into()), &Process, &no_scaffold, &mut Vec::new());
    assert!(r.is_err());
    assert_eq!(ok(fs::read_to_string(&w.file)), "{}");
}

#[test]
fn a_broken_workspace_stops_a_clone_before_it_starts() {
    let w = ws("nope");
    let (_u, url) = upstream_repo();
    assert!(add(&w.file, "lib", &Source::Clone(url), &Process, &no_scaffold, &mut Vec::new()).is_err());
    assert!(!w.dir.join("lib").exists());
}

#[test]
fn init_scaffolds_then_adds() {
    let w = ws("{}");
    let seen = RefCell::new(None);
    let scaffold = |kind: &str, at: &Path| -> Result<()> {
        *seen.borrow_mut() = Some((kind.to_string(), at.to_path_buf()));
        fs::create_dir_all(at)?;
        Ok(())
    };
    let mut out = Vec::new();
    ok(add(&w.file, "svc", &Source::Init("package/rust".into()), &Process, &scaffold, &mut out));
    let out = String::from_utf8_lossy(&out);
    assert_eq!(*seen.borrow(), Some(("package/rust".to_string(), w.dir.join("svc"))));
    assert!(out.starts_with(&format!("==> Scaffolding new package/rust project at {}\n", w.dir.join("svc").display())), "{out}");
    assert_eq!(paths(&w), ["svc"]);
}

#[test]
fn init_over_an_existing_path_is_refused() {
    let w = ws("{}");
    ok(fs::create_dir(w.dir.join("svc")));
    let r = add(&w.file, "svc", &Source::Init("package/rust".into()), &Process, &no_scaffold, &mut Vec::new());
    assert!(why(r).contains("already exists; refusing to scaffold over it"));
}

#[test]
fn a_failing_scaffold_fails_and_the_workspace_is_unchanged() {
    let w = ws("{}");
    let r = add(&w.file, "svc", &Source::Init("nope".into()), &Process, &no_scaffold, &mut Vec::new());
    assert!(r.is_err());
    assert_eq!(ok(fs::read_to_string(&w.file)), "{}");
}

#[test]
fn remove_drops_the_folder_and_keeps_the_directory() {
    let w = ws(r#"{"folders":[{"path":"a"},{"path":"b"}]}"#);
    ok(fs::create_dir(w.dir.join("a")));
    let mut out = Vec::new();
    ok(remove(&w.file, "a", &mut out));
    assert_eq!(String::from_utf8_lossy(&out), "==> Removed folder a from w.code-workspace (subfolder kept on disk).\n");
    assert_eq!(paths(&w), ["b"]);
    assert!(w.dir.join("a").is_dir());
}

#[test]
fn removing_an_unlisted_folder_fails_naming_it() {
    let w = ws(r#"{"folders":[{"path":"a"}]}"#);
    assert_eq!(why(remove(&w.file, "zz", &mut Vec::new())), "zz not in w.code-workspace; nothing to remove.");
}

#[test]
fn removing_from_a_broken_workspace_fails_and_keeps_the_file() {
    let w = ws("nope");
    assert!(remove(&w.file, "a", &mut Vec::new()).is_err());
    assert_eq!(ok(fs::read_to_string(&w.file)), "nope");
}
