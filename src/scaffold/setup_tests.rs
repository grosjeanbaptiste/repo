use super::*;
use crate::fakes::FakeForge;
use std::cell::RefCell;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Records every call; `fail` names a program+first-arg that must fail.
#[derive(Default)]
struct Fake {
    calls: RefCell<Vec<String>>,
    fail: Option<&'static str>,
    config: Vec<(&'static str, &'static str)>,
    forge: FakeForge,
}

impl Exec for Fake {
    fn run(&self, prog: &str, args: &[&str], _cwd: Option<&Path>) -> Result<String> {
        let line = format!("{prog} {}", args.join(" "));
        self.calls.borrow_mut().push(line.clone());
        if self.fail.is_some_and(|f| line.starts_with(f)) {
            bail!("{line} failed");
        }
        if prog == "git" && args.first() == Some(&"config") {
            let hit = self.config.iter().find(|(k, _)| args.get(1) == Some(k));
            return hit.map(|(_, v)| v.to_string()).ok_or_else(|| anyhow::anyhow!("unset"));
        }
        Ok(String::new())
    }
}

fn templates() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir_all(d.path().join("common")));
    ok(fs::write(d.path().join("common/README.md.tmpl"), "# {{project_name}} by {{user}} {{email}} {{year}}"));
    ok(fs::create_dir_all(d.path().join("package/rust")));
    ok(fs::write(d.path().join("package/rust/Cargo.toml.tmpl"), "name = \"{{project_name}}\""));
    d
}

fn run(fake: &Fake, kind: &str, name: &str, no_push: bool) -> (Result<PathBuf>, tempfile::TempDir) {
    let t = templates();
    let parent = ok(tempfile::tempdir());
    let s = Setup { templates: t.path(), parent: parent.path(), kind, name, no_push, year: "2026".into() };
    (setup(&s, fake, &fake.forge), parent)
}

fn fake() -> Fake {
    Fake { config: vec![("user.name", "Ann"), ("user.email", "a@b.c")], ..Fake::default() }
}

#[test]
fn full_sequence_matches_dotfiles_project_setup() {
    let f = fake();
    let (r, parent) = run(&f, "package/rust", "ifdck", false);
    let target = ok(r);
    let calls = f.calls.borrow();
    assert_eq!(calls[0], "git --version");
    assert_eq!(calls[1], "git init -q -b main");
    assert_eq!(calls[calls.len() - 2], "git add -A");
    assert_eq!(calls[calls.len() - 1], "git commit -q -m initial scaffold for ifdck (package/rust)");
    assert_eq!(f.forge.log(), ["available".to_string(), format!("create_repo ifdck private push {}", target.display())]);
    assert_eq!(target, parent.path().join("ifdck"));
    assert_eq!(ok(fs::read_to_string(target.join("README.md"))), "# ifdck by Ann a@b.c 2026");
}

#[test]
fn no_push_never_calls_the_platform() {
    let f = fake();
    ok(run(&f, "package/rust", "ifdck", true).0);
    assert!(f.forge.log().is_empty(), "{:?}", f.forge.log());
}

#[test]
fn unknown_type_creates_nothing() {
    let f = fake();
    let (r, parent) = run(&f, "package/cobol", "ifdck", true);
    assert_eq!(r.err().map(|e| e.to_string()).as_deref(), Some("Unknown project type: package/cobol"));
    assert!(!parent.path().join("ifdck").exists());
    assert!(f.calls.borrow().is_empty() && f.forge.log().is_empty());
}

#[test]
fn missing_args_are_refused() {
    let f = fake();
    for (k, n) in [("", "x"), ("package/rust", "")] {
        let e = run(&f, k, n, true).0.err().map(|e| e.to_string());
        assert_eq!(e.as_deref(), Some("setup needs <type> and <name>"));
    }
}

#[test]
fn missing_parent_is_refused() {
    let t = templates();
    let s = Setup { templates: t.path(), parent: Path::new("/nonexistent/p"), kind: "package/rust", name: "x", no_push: true, year: "1".into() };
    let f = fake();
    let e = setup(&s, &f, &f.forge).err().map(|e| e.to_string());
    assert_eq!(e.as_deref(), Some("Parent directory does not exist: /nonexistent/p"));
}

#[test]
fn existing_target_even_a_dangling_symlink_is_refused() {
    let t = templates();
    let parent = ok(tempfile::tempdir());
    ok(std::os::unix::fs::symlink("/nonexistent", parent.path().join("x")));
    let s = Setup { templates: t.path(), parent: parent.path(), kind: "package/rust", name: "x", no_push: true, year: "1".into() };
    let f = fake();
    let e = setup(&s, &f, &f.forge).err().map(|e| e.to_string());
    assert!(e.is_some_and(|m| m.starts_with("Target already exists: ")));
    assert!(f.calls.borrow().is_empty() && f.forge.log().is_empty());
}

#[test]
fn git_failure_aborts_before_the_platform_creates_anything() {
    for step in ["git init", "git add", "git commit"] {
        let f = Fake { fail: Some(step), ..fake() };
        let (r, _p) = run(&f, "package/rust", "ifdck", false);
        assert!(r.err().is_some_and(|e| e.to_string().contains("failed")), "{step}");
        assert!(!f.forge.log().iter().any(|c| c.starts_with("create_repo")), "{step}");
    }
}

#[test]
fn a_platform_failure_surfaces_and_keeps_the_local_scaffold() {
    let f = Fake { forge: FakeForge::default().failing("create_repo"), ..fake() };
    let (r, parent) = run(&f, "package/rust", "ifdck", false);
    assert!(r.err().is_some_and(|e| e.to_string().contains("create the hosted repository")));
    assert!(parent.path().join("ifdck/README.md").exists());
}

#[test]
fn user_name_falls_back_to_the_environment_when_git_has_none() {
    let f = Fake::default();
    let (r, _p) = run(&f, "package/rust", "ifdck", true);
    let readme = ok(fs::read_to_string(ok(r).join("README.md")));
    let user = std::env::var("USER").unwrap_or_default();
    assert_eq!(readme, format!("# ifdck by {user}  2026"));
}

#[test]
fn a_missing_tool_is_found_before_anything_is_created() {
    let no_git = || Fake { fail: Some("git --version"), ..fake() };
    let no_platform = || Fake { forge: FakeForge::default().failing("available"), ..fake() };
    for (tool, f, no_push) in [("git", no_git(), true), ("git", no_git(), false), ("platform", no_platform(), false)] {
        let (r, parent) = run(&f, "package/rust", "ifdck", no_push);
        assert!(r.is_err(), "{tool}");
        assert!(!parent.path().join("ifdck").exists(), "{tool}");
    }
}

#[test]
fn the_platform_is_not_needed_with_no_push() {
    let f = Fake { forge: FakeForge::default().failing("available"), ..fake() };
    assert!(run(&f, "package/rust", "ifdck", true).0.is_ok());
}
