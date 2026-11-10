use super::*;
use crate::scaffold::native::Process;
use std::fs;
use std::process::Command;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn git(dir: &Path, args: &[&str]) {
    let out = ok(Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .output());
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn mkrepo(root: &Path, name: &str) -> PathBuf {
    let p = root.join(name);
    ok(fs::create_dir_all(&p));
    git(&p, &["init", "-q", "-b", "main"]);
    ok(fs::write(p.join("f"), "1"));
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-q", "-m", "one"]);
    p
}

fn lines(registry: &Path) -> Vec<String> {
    ok(fs::read_to_string(registry)).lines().map(String::from).collect()
}

struct Fx {
    _d: tempfile::TempDir,
    root: PathBuf,
    registry: PathBuf,
}

fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let root = d.path().join("src");
    ok(fs::create_dir(&root));
    let registry = d.path().join("state/projects.txt");
    Fx { _d: d, root, registry }
}

fn run(f: &Fx) -> Summary {
    ok(scan(&f.registry, std::slice::from_ref(&f.root), &Process))
}

#[test]
fn discovers_repositories_and_creates_the_registry_and_its_directory() {
    let f = fx();
    let (a, b) = (mkrepo(&f.root, "alpha"), mkrepo(&f.root, "beta"));
    let s = run(&f);
    assert_eq!((s.added, s.unchanged, s.removed), (2, 0, 0));
    assert_eq!(lines(&f.registry), [format!("alpha|{}", a.display()), format!("beta|{}", b.display())]);
}

#[test]
fn rerunning_is_idempotent() {
    let f = fx();
    mkrepo(&f.root, "alpha");
    run(&f);
    let first = lines(&f.registry);
    let s = run(&f);
    assert_eq!((s.added, s.unchanged, s.removed), (0, 1, 0));
    assert_eq!(lines(&f.registry), first);
}

#[test]
fn a_stale_entry_is_pruned_and_a_live_one_kept_as_written() {
    let f = fx();
    let live = mkrepo(&f.root, "live");
    ok(fs::create_dir_all(f.registry.parent().unwrap_or(&f.root)));
    ok(fs::write(&f.registry, format!("custom-name|{}\ngone|/nonexistent/gone\n", live.display())));
    let s = run(&f);
    assert_eq!((s.added, s.unchanged, s.removed), (0, 1, 1));
    assert_eq!(lines(&f.registry), [format!("custom-name|{}", live.display())]);
}

#[test]
fn comments_in_the_registry_do_not_survive_a_rewrite_but_entries_do() {
    let f = fx();
    let live = mkrepo(&f.root, "live");
    ok(fs::create_dir_all(f.registry.parent().unwrap_or(&f.root)));
    ok(fs::write(&f.registry, format!("# note\n\nlive|{}\n", live.display())));
    run(&f);
    assert_eq!(lines(&f.registry), [format!("live|{}", live.display())]);
}

#[test]
fn a_repo_found_under_two_overlapping_roots_is_added_once() {
    let f = fx();
    mkrepo(&f.root, "alpha");
    let s = ok(scan(&f.registry, &[f.root.clone(), f.root.clone()], &Process));
    assert_eq!((s.added, s.unchanged), (1, 1));
    assert_eq!(lines(&f.registry).len(), 1);
}

#[test]
fn a_repo_whose_linked_worktree_is_registered_is_not_added_beside_it() {
    let f = fx();
    let main = mkrepo(&f.root, "main-checkout");
    let wt_dir = ok(tempfile::tempdir());
    let wt = wt_dir.path().join("feature");
    git(&main, &["worktree", "add", "-q", "-b", "feature", &wt.display().to_string()]);
    ok(fs::create_dir_all(f.registry.parent().unwrap_or(&f.root)));
    ok(fs::write(&f.registry, format!("proj|{}\n", wt.display())));
    let s = run(&f);
    assert_eq!(s.added, 0);
    assert_eq!(lines(&f.registry), [format!("proj|{}", wt.display())]);
}

#[test]
fn a_repo_with_an_unregistered_worktree_is_still_added() {
    let f = fx();
    let main = mkrepo(&f.root, "main-checkout");
    let wt_dir = ok(tempfile::tempdir());
    git(&main, &["worktree", "add", "-q", "-b", "feature", &wt_dir.path().join("w").display().to_string()]);
    assert_eq!(run(&f).added, 1);
}

#[test]
fn a_symlinked_registry_spelling_still_matches_the_physical_worktree() {
    let f = fx();
    let main = mkrepo(&f.root, "main-checkout");
    let wt_dir = ok(tempfile::tempdir());
    let wt = wt_dir.path().join("feature");
    git(&main, &["worktree", "add", "-q", "-b", "feature", &wt.display().to_string()]);
    let link = f.root.join("..").join("alias");
    ok(std::os::unix::fs::symlink(&wt, &link));
    ok(fs::create_dir_all(f.registry.parent().unwrap_or(&f.root)));
    ok(fs::write(&f.registry, format!("proj|{}\n", link.display())));
    assert_eq!(run(&f).added, 0);
}

#[test]
fn a_malformed_registry_fails_loudly_and_is_left_untouched() {
    let f = fx();
    mkrepo(&f.root, "alpha");
    ok(fs::create_dir_all(f.registry.parent().unwrap_or(&f.root)));
    ok(fs::write(&f.registry, "garbage\n"));
    let r = scan(&f.registry, std::slice::from_ref(&f.root), &Process);
    assert!(r.err().is_some_and(|e| format!("{e:#}").contains("name|path")));
    assert_eq!(lines(&f.registry), ["garbage"]);
}

#[test]
fn unreadable_directories_become_warnings() {
    use std::os::unix::fs::PermissionsExt;
    let f = fx();
    mkrepo(&f.root, "alpha");
    let locked = f.root.join("locked");
    ok(fs::create_dir(&locked));
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o0)));
    let s = run(&f);
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)));
    assert_eq!(s.added, 1);
    assert!(s.warnings.iter().any(|w| w.contains("locked")), "{:?}", s.warnings);
}

#[test]
fn a_failing_worktree_listing_warns_and_still_adds() {
    struct NoWorktrees;
    impl Exec for NoWorktrees {
        fn run(&self, p: &str, a: &[&str], c: Option<&Path>) -> Result<String> {
            if a.contains(&"worktree") {
                anyhow::bail!("git worktree list failed");
            }
            Process.run(p, a, c)
        }
    }
    let f = fx();
    mkrepo(&f.root, "alpha");
    let s = ok(scan(&f.registry, std::slice::from_ref(&f.root), &NoWorktrees));
    assert_eq!(s.added, 1);
    assert!(s.warnings.iter().any(|w| w.contains("worktree")), "{:?}", s.warnings);
}

#[test]
fn no_temporary_file_is_left_behind() {
    let f = fx();
    mkrepo(&f.root, "alpha");
    run(&f);
    let dir = f.registry.parent().unwrap_or(&f.root);
    let names: Vec<_> = ok(fs::read_dir(dir)).filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["projects.txt"]);
}

#[test]
fn the_message_matches_the_bash_wording_with_a_symbol_not_an_emoji() {
    let s = Summary { added: 2, unchanged: 3, removed: 1, warnings: vec![] };
    assert_eq!(s.message(Path::new("/r/projects.txt")), "✓ Scan complete: 2 added, 3 unchanged, 1 removed (stale paths). Registry: /r/projects.txt");
}
