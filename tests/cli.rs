//! Argument handling of the real binary. No network: clap refuses before any call.
use std::process::{Command, Output};

fn repo(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repo"))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run repo: {e}"))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[test]
fn name_and_generation_flags_are_mutually_exclusive() {
    for extra in [["--prefix", "act"], ["--domain", "git"]] {
        let out = repo(&["new", "package/rust", "--name", "ifdck", extra[0], extra[1]]);
        assert_eq!(out.status.code(), Some(2), "{extra:?}");
        assert!(
            stderr(&out).contains("cannot be used with"),
            "{}",
            stderr(&out)
        );
    }
}

#[test]
fn new_needs_a_name_or_a_prefix_and_domain() {
    for args in [
        vec!["new", "package/rust"],
        vec!["new", "package/rust", "--prefix", "act"],
    ] {
        let out = repo(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).contains("required"), "{}", stderr(&out));
    }
}

#[test]
fn new_needs_a_project_type() {
    assert_eq!(repo(&["new", "--name", "ifdck"]).status.code(), Some(2));
}

#[test]
fn invalid_names_fail_before_any_network_or_file_work() {
    let root = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let root_arg = root.path().to_str().unwrap_or("");
    for bad in ["..", "a/b", "a b"] {
        let out = repo(&[
            "new",
            "package/rust",
            &format!("--name={bad}"),
            "--root",
            root_arg,
        ]);
        assert_eq!(out.status.code(), Some(1), "{bad:?}");
        assert!(
            stderr(&out).contains("not a valid"),
            "{bad:?}: {}",
            stderr(&out)
        );
    }
    assert_eq!(
        std::fs::read_dir(root.path())
            .map(|d| d.count())
            .unwrap_or(1),
        0
    );
}
