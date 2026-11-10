//! Real pipeline for `ifdck`: actit -> gh (read only) -> `dotfiles project setup --no-push`
//! into temp directories. Nothing is created on GitHub. Needs gh, dotfiles and network:
//! `dotfiles heavy run --medium -- cargo test --test live -- --ignored`.
use std::path::Path;
use std::process::{Command, Output};

const COMMON: [&str; 5] = [".git", "README.md", "AGENTS.md", "CONTEXT.md", "LICENSE"];

fn repo(args: &[&str], root: &Path) -> Output {
    run_with(args, root, None)
}

fn run_with(args: &[&str], root: &Path, path: Option<&str>) -> Output {
    run_env(args, root, path.map(|p| ("PATH", p)))
}

fn run_env(args: &[&str], root: &Path, env: Option<(&str, &str)>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_repo"));
    cmd.args(args).args(["--no-push", "--root"]).arg(root);
    if let Some((key, value)) = env {
        cmd.env(key, value);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("cannot run repo: {e}"))
}

fn log(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr)
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"))
}

fn is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|d| d.count() == 0)
        .unwrap_or(false)
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn ifdck_is_checked_and_scaffolded_locally() {
    let root = tmp();
    let out = repo(&["new", "package/rust", "--name", "ifdck"], root.path());
    assert!(out.status.success(), "{}", log(&out));
    for file in COMMON.iter().chain(&["Cargo.toml", "src/lib.rs"]) {
        assert!(
            root.path().join("ifdck").join(file).exists(),
            "missing {file}\n{}",
            log(&out)
        );
    }
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn every_known_project_type_scaffolds_ifdck() {
    for kind in [
        "app/python",
        "app/rust",
        "doc/tex",
        "mobile/native",
        "package/python",
        "package/rust",
        "workspace",
    ] {
        let root = tmp();
        let out = repo(&["new", kind, "--name", "ifdck"], root.path());
        assert!(out.status.success(), "{kind}: {}", log(&out));
        assert!(
            root.path().join("ifdck/.git").exists(),
            "{kind}: no repository\n{}",
            log(&out)
        );
    }
}

#[test]
#[ignore = "needs gh and network"]
fn a_taken_name_is_refused_and_nothing_is_created() {
    let root = tmp();
    let out = repo(&["new", "package/rust", "--name", "idk"], root.path());
    assert_eq!(out.status.code(), Some(1));
    assert!(log(&out).contains("idk is taken"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn running_twice_refuses_the_second_time_and_keeps_the_first() {
    let root = tmp();
    assert!(
        repo(&["new", "package/rust", "--name", "ifdck"], root.path())
            .status
            .success()
    );
    let before = std::fs::read_to_string(root.path().join("ifdck/Cargo.toml")).unwrap_or_default();
    let again = repo(&["new", "package/rust", "--name", "ifdck"], root.path());
    assert_eq!(again.status.code(), Some(1));
    assert!(log(&again).contains("ifdck is taken"), "{}", log(&again));
    let after = std::fs::read_to_string(root.path().join("ifdck/Cargo.toml")).unwrap_or_default();
    assert_eq!(before, after);
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn an_unknown_project_type_fails_loudly_without_a_directory() {
    let root = tmp();
    let out = repo(&["new", "nonsense/kind", "--name", "ifdck"], root.path());
    assert!(!out.status.success(), "{}", log(&out));
    assert!(log(&out).contains("Unknown project type"), "{}", log(&out));
    assert!(
        !root.path().join("ifdck").exists(),
        "partial scaffold left behind"
    );
}

#[test]
#[ignore = "needs gh and network"]
fn a_generated_name_dry_run_prints_one_free_act_name() {
    let root = tmp();
    let out = repo(
        &[
            "new",
            "package/rust",
            "--prefix",
            "act",
            "--domain",
            "git repositories tooling",
            "--dry-run",
        ],
        root.path(),
    );
    assert!(out.status.success(), "{}", log(&out));
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(
        name.starts_with("act") && !name.contains(char::is_whitespace),
        "{name:?}"
    );
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs gh and network"]
fn a_domain_actname_does_not_know_fails_loudly() {
    let root = tmp();
    let out = repo(
        &[
            "new",
            "package/rust",
            "--prefix",
            "act",
            "--domain",
            "zzqq xxyy",
            "--dry-run",
        ],
        root.path(),
    );
    assert!(!out.status.success());
    assert!(log(&out).contains("no free name"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs gh and network"]
fn missing_gh_fails_loudly_and_creates_nothing() {
    let (root, empty_path) = (tmp(), tmp());
    let out = run_with(
        &["new", "package/rust", "--name", "ifdck"],
        root.path(),
        empty_path.path().to_str(),
    );
    assert!(!out.status.success());
    assert!(log(&out).contains("gh"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs gh and network"]
fn missing_git_fails_loudly_and_creates_nothing() {
    let (root, bin) = (tmp(), tmp());
    let gh = Command::new("which")
        .arg("gh")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    assert!(
        std::os::unix::fs::symlink(&gh, bin.path().join("gh")).is_ok(),
        "gh not found"
    );
    let out = run_with(
        &["new", "package/rust", "--name", "ifdck"],
        root.path(),
        bin.path().to_str(),
    );
    assert!(!out.status.success());
    assert!(log(&out).contains("git"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs gh and network"]
fn missing_dotfiles_fails_loudly_with_via_dotfiles() {
    let (root, bin) = (tmp(), tmp());
    let gh = Command::new("which")
        .arg("gh")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    assert!(
        std::os::unix::fs::symlink(&gh, bin.path().join("gh")).is_ok(),
        "gh not found"
    );
    let out = run_with(
        &["new", "package/rust", "--name", "ifdck", "--via-dotfiles"],
        root.path(),
        bin.path().to_str(),
    );
    assert!(!out.status.success());
    assert!(log(&out).contains("dotfiles"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs dotfiles"]
fn an_unknown_command_is_forwarded_to_dotfiles_and_fails() {
    let out = Command::new(env!("CARGO_BIN_EXE_repo"))
        .arg("definitely-not-a-command")
        .output()
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!out.status.success());
    assert!(
        log(&out).contains("dotfiles project definitely-not-a-command"),
        "{}",
        log(&out)
    );
}

#[test]
#[ignore = "needs gh and network"]
fn a_dangling_symlink_is_refused_as_taken_and_left_alone() {
    let root = tmp();
    assert!(std::os::unix::fs::symlink("/nonexistent", root.path().join("ifdck")).is_ok());
    let out = repo(&["new", "package/rust", "--name", "ifdck"], root.path());
    assert_eq!(out.status.code(), Some(1));
    assert!(log(&out).contains("ifdck is taken"), "{}", log(&out));
    assert!(std::fs::symlink_metadata(root.path().join("ifdck")).is_ok());
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn a_missing_root_or_a_file_as_root_fails_loudly_and_creates_nothing() {
    let base = tmp();
    let missing = base.path().join("a/b/c");
    let out = repo(&["new", "package/rust", "--name", "ifdck"], &missing);
    assert!(!out.status.success(), "{}", log(&out));
    assert!(
        log(&out).contains("Parent directory does not exist"),
        "{}",
        log(&out)
    );
    assert!(!missing.exists());
    let file = base.path().join("file");
    assert!(std::fs::write(&file, b"x").is_ok());
    let out = repo(&["new", "package/rust", "--name", "ifdck"], &file);
    assert!(!out.status.success(), "{}", log(&out));
    assert!(file.is_file());
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn a_relative_root_works() {
    let base = tmp();
    assert!(std::fs::create_dir(base.path().join("rel")).is_ok());
    let out = Command::new(env!("CARGO_BIN_EXE_repo"))
        .current_dir(base.path())
        .args([
            "new",
            "package/rust",
            "--name",
            "ifdck",
            "--no-push",
            "--root",
            "rel",
        ])
        .output()
        .unwrap_or_else(|e| panic!("cannot run repo: {e}"));
    assert!(out.status.success(), "{}", log(&out));
    assert!(base.path().join("rel/ifdck/.git").exists());
}

#[test]
#[ignore = "needs gh, dotfiles and network"]
fn two_simultaneous_runs_leave_exactly_one_clean_repository() {
    let root = tmp();
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_repo"))
            .args([
                "new",
                "package/rust",
                "--name",
                "ifdck",
                "--no-push",
                "--root",
            ])
            .arg(root.path())
            .output()
            .unwrap_or_else(|e| panic!("cannot run repo: {e}"))
    };
    let (a, b) = std::thread::scope(|s| {
        let (ha, hb) = (s.spawn(spawn), s.spawn(spawn));
        (
            ha.join().unwrap_or_else(|_| panic!("thread")),
            hb.join().unwrap_or_else(|_| panic!("thread")),
        )
    });
    let wins = [&a, &b].iter().filter(|o| o.status.success()).count();
    assert_eq!(wins, 1, "A: {}\nB: {}", log(&a), log(&b));
    let count = Command::new("git")
        .args(["-C"])
        .arg(root.path().join("ifdck"))
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    assert_eq!(
        count, "1",
        "the repository must hold exactly one scaffold commit"
    );
}

#[test]
#[ignore = "needs gh"]
fn an_invalid_github_token_fails_loudly_and_creates_nothing() {
    let root = tmp();
    let out = run_env(
        &["new", "package/rust", "--name", "ifdck"],
        root.path(),
        Some(("GH_TOKEN", "invalid")),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(log(&out).contains("Bad credentials"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs gh"]
fn being_offline_fails_loudly_and_creates_nothing() {
    let root = tmp();
    let out = run_env(
        &["new", "package/rust", "--name", "ifdck"],
        root.path(),
        Some(("HTTPS_PROXY", "http://127.0.0.1:9")),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(log(&out).contains("github:"), "{}", log(&out));
    assert!(is_empty(root.path()));
}

#[test]
#[ignore = "needs dotfiles"]
fn a_dotfiles_project_command_that_succeeds_is_forwarded_with_its_output() {
    let out = Command::new(env!("CARGO_BIN_EXE_repo"))
        .arg("list")
        .output()
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(out.status.success(), "{}", log(&out));
}

#[test]
#[ignore = "needs gh and network"]
fn the_project_type_can_be_piped_on_stdin() {
    use std::io::Write;
    use std::process::Stdio;
    let root = tmp();
    let mut child = Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(["new", "--name", "ifdck", "--no-push", "--root"])
        .arg(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot run repo: {e}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("no stdin"));
    assert!(stdin.write_all(b"package/rust\n").is_ok());
    drop(stdin);
    let out = child.wait_with_output().unwrap_or_else(|e| panic!("{e}"));
    assert!(out.status.success(), "{}", log(&out));
    assert!(root.path().join("ifdck/.git").is_dir());
}
