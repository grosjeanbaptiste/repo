use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(dir: &Path, rel: &str, mode: u32) -> PathBuf {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        ok(fs::create_dir_all(parent));
    }
    ok(fs::write(&p, "x"));
    ok(fs::set_permissions(&p, fs::Permissions::from_mode(mode)));
    p
}

fn why<T: std::fmt::Debug>(r: Result<T>) -> String {
    r.err().map(|e| e.to_string()).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn the_conventional_script_is_found() {
    let d = ok(tempfile::tempdir());
    let p = put(d.path(), "stores/scripts/tf_release.py", 0o644);
    assert_eq!(ok(resolve(d.path(), None)), p);
}

#[test]
fn an_override_wins_and_a_relative_one_is_taken_from_the_target() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "stores/scripts/tf_release.py", 0o644);
    let mine = put(d.path(), "ci/ship.sh", 0o644);
    assert_eq!(ok(resolve(d.path(), Some("ci/ship.sh"))), mine);
    let other = ok(tempfile::tempdir());
    let abs = put(other.path(), "abs.sh", 0o644);
    assert_eq!(ok(resolve(d.path(), Some(&abs.to_string_lossy()))), abs);
}

#[test]
fn no_script_says_where_it_looked_and_how_to_override() {
    let d = ok(tempfile::tempdir());
    let m = why(resolve(d.path(), None));
    assert_eq!(
        m,
        format!(
            "no release script found in {}\n  Tried: stores/scripts/tf_release.py\n  Override with --script PATH if yours lives elsewhere.",
            d.path().display()
        )
    );
}

#[test]
fn an_override_that_does_not_exist_is_the_same_error_not_a_silent_fallback() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "stores/scripts/tf_release.py", 0o644);
    assert!(why(resolve(d.path(), Some("nope.sh"))).starts_with("no release script found in "));
}

#[test]
fn an_unreadable_convention_is_not_found() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "stores/scripts/tf_release.py", 0o000);
    assert!(why(resolve(d.path(), None)).starts_with("no release script found in "));
}

#[test]
fn python_and_shell_scripts_get_their_interpreter() {
    let d = ok(tempfile::tempdir());
    let py = put(d.path(), "a.py", 0o644);
    let sh = put(d.path(), "b.sh", 0o644);
    let bash = put(d.path(), "c.bash", 0o644);
    assert_eq!(ok(command(&py)), ["python3".to_string(), py.to_string_lossy().into_owned()]);
    assert_eq!(ok(command(&sh))[0], "bash");
    assert_eq!(ok(command(&bash))[0], "bash");
}

#[test]
fn any_other_executable_runs_itself() {
    let d = ok(tempfile::tempdir());
    let x = put(d.path(), "ship", 0o755);
    assert_eq!(ok(command(&x)), [x.to_string_lossy().into_owned()]);
}

#[test]
fn a_file_that_is_neither_known_nor_executable_is_refused_naming_it() {
    let d = ok(tempfile::tempdir());
    let x = put(d.path(), "ship", 0o644);
    assert_eq!(
        why(command(&x)),
        format!("cannot determine how to run {} (not .py/.sh, not executable)", x.display())
    );
}
