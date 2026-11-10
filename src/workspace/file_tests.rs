use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// A temp directory with its symlinks resolved (macOS /var is /private/var), as `find` reports it.
fn tmp() -> (tempfile::TempDir, PathBuf) {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    (d, root)
}

fn names(json: &str) -> Vec<(String, String)> {
    ok(folders(json)).into_iter().map(|f| (f.name, f.path)).collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

#[test]
fn folders_use_the_name_else_the_path() {
    let j = r#"{"folders":[{"name":"api","path":"a"},{"path":"b"},{"name":"","path":"c"}]}"#;
    assert_eq!(names(j), [pair("api", "a"), pair("b", "b"), pair("", "c")]);
}

#[test]
fn a_null_name_falls_back_to_the_path() {
    assert_eq!(names(r#"{"folders":[{"name":null,"path":"a"}]}"#), [pair("a", "a")]);
}

#[test]
fn no_folders_key_or_an_empty_array_is_no_folder() {
    assert!(names("{}").is_empty());
    assert!(names(r#"{"folders":[]}"#).is_empty());
    assert!(names(r#"{"folders":null}"#).is_empty());
}

#[test]
fn malformed_workspaces_fail_loudly() {
    for bad in [
        "",
        "not json",
        "// comment\n{}",
        "[]",
        r#"{"folders":{}}"#,
        r#"{"folders":["a"]}"#,
        r#"{"folders":[{"name":"x"}]}"#,
        r#"{"folders":[{"path":3}]}"#,
    ] {
        assert!(folders(bad).is_err(), "{bad:?} accepted");
    }
}

#[test]
fn the_error_names_the_bad_entry() {
    let why = folders(r#"{"folders":[{"path":"a"},{"name":"x"}]}"#).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(why.contains("folders[1]") && why.contains("path"), "{why}");
}

#[test]
fn add_appends_in_order_with_two_space_indent_and_a_final_newline() {
    let out = ok(add(r#"{"folders":[{"path":"a"}],"settings":{"x":1}}"#, "b", "b")).unwrap_or_default();
    let want = "{\n  \"folders\": [\n    {\n      \"path\": \"a\"\n    },\n    {\n      \"name\": \"b\",\n      \"path\": \"b\"\n    }\n  ],\n  \"settings\": {\n    \"x\": 1\n  }\n}\n";
    assert_eq!(out, want);
}

#[test]
fn add_creates_the_folders_array_when_missing() {
    let out = ok(add("{}", "a", "a")).unwrap_or_default();
    assert_eq!(names(&out), [pair("a", "a")]);
}

#[test]
fn add_keeps_other_keys_in_their_order() {
    let out = ok(add(r#"{"z":1,"folders":[],"a":2}"#, "n", "n")).unwrap_or_default();
    let (z, f, a) = (out.find("\"z\""), out.find("\"folders\""), out.find("\"a\""));
    assert!(z < f && f < a, "{out}");
}

#[test]
fn add_is_a_no_op_when_the_path_is_there() {
    assert_eq!(ok(add(r#"{"folders":[{"path":"a"}]}"#, "other-name", "a")), None);
}

#[test]
fn add_refuses_a_non_array_folders_and_a_non_object_root() {
    assert!(add(r#"{"folders":"x"}"#, "a", "a").is_err());
    assert!(add("[]", "a", "a").is_err());
    assert!(add("nope", "a", "a").is_err());
}

#[test]
fn remove_drops_every_entry_with_the_path() {
    let j = r#"{"folders":[{"path":"a"},{"name":"x","path":"b"},{"name":"y","path":"a"}]}"#;
    let out = ok(remove(j, "a")).unwrap_or_default();
    assert_eq!(names(&out), [pair("x", "b")]);
    assert!(out.ends_with("}\n"));
}

#[test]
fn remove_matches_the_path_not_the_name() {
    assert_eq!(ok(remove(r#"{"folders":[{"name":"api","path":"a"}]}"#, "api")), None);
}

#[test]
fn remove_of_an_absent_path_is_none_and_of_a_broken_file_an_error() {
    assert_eq!(ok(remove("{}", "a")), None);
    assert!(remove("nope", "a").is_err());
}

#[test]
fn find_returns_the_workspace_of_the_directory_itself() {
    let (_d, root) = tmp();
    ok(fs::write(root.join("x.code-workspace"), "{}"));
    assert_eq!(ok(find(&root)), Some(root.join("x.code-workspace")));
}

#[test]
fn find_climbs_to_an_ancestor() {
    let (_d, root) = tmp();
    ok(fs::write(root.join("x.code-workspace"), "{}"));
    let deep = root.join("a/b");
    ok(fs::create_dir_all(&deep));
    assert_eq!(ok(find(&deep)), Some(root.join("x.code-workspace")));
}

#[test]
fn find_takes_the_nearest_directory_and_the_first_name_in_it() {
    let (_d, root) = tmp();
    ok(fs::write(root.join("far.code-workspace"), "{}"));
    let near = root.join("near");
    ok(fs::create_dir_all(&near));
    ok(fs::write(near.join("b.code-workspace"), "{}"));
    ok(fs::write(near.join("a.code-workspace"), "{}"));
    assert_eq!(ok(find(&near)), Some(near.join("a.code-workspace")));
}

#[test]
fn find_ignores_a_directory_named_like_a_workspace_and_other_files() {
    let (_d, root) = tmp();
    ok(fs::create_dir(root.join("x.code-workspace")));
    ok(fs::write(root.join("y.json"), "{}"));
    assert_eq!(ok(find(&root)), None);
}

#[test]
fn find_without_a_workspace_is_none_and_a_missing_start_an_error() {
    let (_d, root) = tmp();
    assert_eq!(ok(find(&root)), None);
    assert!(find(&root.join("nope")).is_err());
}
