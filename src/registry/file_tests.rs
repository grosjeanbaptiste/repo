use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn env(pairs: &'static [(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

fn e(name: &str, path: &str) -> Entry {
    Entry { name: name.into(), path: path.into() }
}

#[test]
fn path_follows_the_state_root_then_home() {
    assert_eq!(ok(path(env(&[("DOTFILES_STATE_ROOT", "/s"), ("HOME", "/h")]))), PathBuf::from("/s/projects.txt"));
    assert_eq!(ok(path(env(&[("HOME", "/h")]))), PathBuf::from("/h/machine/projects.txt"));
}

#[test]
fn an_empty_state_root_falls_back_to_home() {
    assert_eq!(ok(path(env(&[("DOTFILES_STATE_ROOT", ""), ("HOME", "/h")]))), PathBuf::from("/h/machine/projects.txt"));
}

#[test]
fn no_state_root_and_no_home_fails_loudly() {
    let r = path(env(&[]));
    assert_eq!(r.err().map(|e| e.to_string()).as_deref(), Some("HOME is not set"));
}

#[test]
fn parse_drops_comments_and_blank_lines() {
    let text = "# header\n\na|/x\n   # indented comment\n  \nb|/y z\n";
    assert_eq!(ok(parse(text)), [e("a", "/x"), e("b", "/y z")]);
}

#[test]
fn a_malformed_row_fails_loudly_naming_its_line() {
    for bad in ["nopipe", "|/nameless", "c|", " | "] {
        let r = parse(&format!("a|/x\n{bad}\n"));
        let msg = r.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(msg.contains("line 2") && msg.contains("name|path"), "{bad:?}: {msg}");
    }
}

#[test]
fn parse_keeps_pipes_inside_the_path() {
    assert_eq!(ok(parse("a|/x|y\n")), [e("a", "/x|y")]);
}

#[test]
fn parse_handles_crlf_and_a_missing_final_newline() {
    assert_eq!(ok(parse("a|/x\r\nb|/y")), [e("a", "/x"), e("b", "/y")]);
}

#[test]
fn read_of_a_missing_file_is_an_empty_registry() {
    assert_eq!(ok(read(Path::new("/nonexistent/projects.txt"))), []);
}

#[test]
fn read_of_an_unreadable_file_fails_loudly() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("projects.txt"))); // a directory is not a registry
    let r = read(&d.path().join("projects.txt"));
    assert!(r.err().is_some_and(|e| format!("{e:#}").contains("projects.txt")));
}

#[test]
fn read_parses_the_file() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("p"), "a|/x\n"));
    assert_eq!(ok(read(&d.path().join("p"))), [e("a", "/x")]);
}

#[test]
fn find_is_an_exact_name_match_and_first_wins() {
    let v = [e("app", "/1"), e("app2", "/2"), e("app", "/3")];
    assert_eq!(find(&v, "app"), Some(PathBuf::from("/1")));
    assert_eq!(find(&v, "ap"), None);
    assert_eq!(find(&v, "APP"), None);
    assert_eq!(find(&[], "app"), None);
}

#[test]
fn list_is_sorted_padded_to_30_and_flags_missing_paths() {
    let d = ok(tempfile::tempdir());
    let here = d.path().display().to_string();
    let v = [e("zeta", "/nonexistent/z"), e("alpha", &here)];
    assert_eq!(list(&v), format!("{:<30} {here}\n{:<30} /nonexistent/z (missing)\n", "alpha", "zeta"));
}

#[test]
fn list_of_nothing_is_empty_text() {
    assert_eq!(list(&[]), "");
}

#[test]
fn a_name_longer_than_30_is_not_truncated() {
    let long = "n".repeat(35);
    assert_eq!(list(&[e(&long, "/nonexistent")]), format!("{long} /nonexistent (missing)\n"));
}

#[test]
fn list_sorts_case_insensitively_with_the_exact_name_as_tie_break() {
    let v = [e("beta", "/1"), e("Gamma", "/2"), e("alpha", "/3"), e("Alpha", "/4")];
    let names: Vec<String> = list(&v).lines().map(|l| l.split_whitespace().next().unwrap_or("").to_string()).collect();
    assert_eq!(names, ["Alpha", "alpha", "beta", "Gamma"]);
}
