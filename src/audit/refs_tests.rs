use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(d) = p.parent() {
        ok(fs::create_dir_all(d));
    }
    ok(fs::write(p, text));
}

fn refs(root: &Path) -> Vec<(String, String)> {
    let mut w = Vec::new();
    let r = ok(references(root, &mut w));
    assert!(w.is_empty(), "{w:?}");
    r.into_iter().map(|r| (r.var, r.source)).collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

#[test]
fn an_empty_tree_references_nothing() {
    assert!(refs(ok(tempfile::tempdir()).path()).is_empty());
}

#[test]
fn the_three_kinds_of_source_are_read() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".env.example", "DB=1\n");
    put(d.path(), ".github/workflows/ci.yml", "x: ${{ secrets.TOKEN }}\n");
    put(d.path(), "docker-compose.yml", "x: ${PORT:-80}\n");
    assert_eq!(refs(d.path()), [pair("DB", ".env.example"), pair("PORT", "docker-compose.yml"), pair("TOKEN", ".github/workflows/ci.yml")]);
}

#[test]
fn every_env_template_name_is_read() {
    let d = ok(tempfile::tempdir());
    for (i, n) in [".env.example", ".env.template", ".env.sample", ".env.dist"].iter().enumerate() {
        put(d.path(), n, &format!("V{i}=1\n"));
    }
    put(d.path(), ".env", "REAL=secret\n");
    put(d.path(), ".env.local", "LOCAL=1\n");
    let vars: Vec<String> = refs(d.path()).into_iter().map(|r| r.0).collect();
    assert_eq!(vars, ["V0", "V1", "V2", "V3"]);
}

#[test]
fn env_templates_are_found_down_to_four_levels() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "a/b/c/.env.example", "OK4=1\n");
    put(d.path(), "a/b/c/d/.env.example", "TOO_DEEP=1\n");
    assert_eq!(refs(d.path()), [pair("OK4", "a/b/c/.env.example")]);
}

#[test]
fn compose_files_are_found_down_to_three_levels() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "a/b/Dockerfile", "ENV X=${OK3}\n");
    put(d.path(), "a/b/c/Dockerfile", "ENV X=${TOO_DEEP}\n");
    assert_eq!(refs(d.path()), [pair("OK3", "a/b/Dockerfile")]);
}

#[test]
fn every_compose_and_dockerfile_name_is_read() {
    let d = ok(tempfile::tempdir());
    let names = ["docker-compose.yml", "docker-compose.prod.yaml", "Dockerfile", "Dockerfile.dev", "compose.yml", "compose.yaml"];
    for (i, n) in names.iter().enumerate() {
        put(d.path(), n, &format!("${{C{i}}}\n"));
    }
    put(d.path(), "Dockerfiles", "${NOT_A_DOCKERFILE}\n");
    put(d.path(), "docker-compose.txt", "${NOT_COMPOSE}\n");
    assert_eq!(refs(d.path()).len(), names.len());
}

#[test]
fn workflows_are_found_at_any_depth_with_yml_or_yaml_only() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".github/workflows/a.yml", "${{ secrets.A }}");
    put(d.path(), ".github/workflows/sub/deep/b.yaml", "${{ vars.B }}");
    put(d.path(), ".github/workflows/c.txt", "${{ secrets.C }}");
    put(d.path(), "other/.github/workflows/d.yml", "${{ secrets.D }}");
    let vars: Vec<String> = refs(d.path()).into_iter().map(|r| r.0).collect();
    assert_eq!(vars, ["A", "B"]);
}

#[test]
fn a_variable_in_several_files_lists_each_source_once_sorted() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "b/.env.example", "X=1\nX=2\n");
    put(d.path(), "a/.env.example", "X=1\n");
    assert_eq!(refs(d.path()), [pair("X", "a/.env.example"), pair("X", "b/.env.example")]);
}

#[test]
fn variables_sort_case_insensitively_with_exact_name_as_tiebreak() {
    let d = ok(tempfile::tempdir());
    put(d.path(), ".env.example", "b=1\nA=1\nB=1\na=1\nC=1\n");
    let vars: Vec<String> = refs(d.path()).into_iter().map(|r| r.0).collect();
    assert_eq!(vars, ["A", "a", "B", "b", "C"]);
}

#[test]
fn symlinks_are_not_followed_to_files_or_directories() {
    let d = ok(tempfile::tempdir());
    let outside = ok(tempfile::tempdir());
    put(outside.path(), ".env.example", "OUT=1\n");
    put(d.path(), "real/.env.example", "REAL=1\n");
    ok(std::os::unix::fs::symlink(outside.path(), d.path().join("link")));
    ok(std::os::unix::fs::symlink(outside.path().join(".env.example"), d.path().join(".env.sample")));
    assert_eq!(refs(d.path()), [pair("REAL", "real/.env.example")]);
}

#[test]
fn a_directory_named_like_a_file_is_ignored() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("Dockerfile")));
    assert!(refs(d.path()).is_empty());
}

#[test]
fn file_names_with_spaces_in_the_path_work() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "my app/.env.example", "S=1\n");
    assert_eq!(refs(d.path()), [pair("S", "my app/.env.example")]);
}

#[test]
fn non_utf8_content_is_read_lossily() {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join(".env.example"), b"A=\xff\xfe\nB=1\n"));
    let vars: Vec<String> = refs(d.path()).into_iter().map(|r| r.0).collect();
    assert_eq!(vars, ["A", "B"]);
}

#[test]
fn an_unreadable_directory_is_a_warning_and_the_rest_is_still_read() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    put(d.path(), "ok/.env.example", "OK=1\n");
    put(d.path(), "locked/.env.example", "HIDDEN=1\n");
    let locked = d.path().join("locked");
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)));
    let mut w = Vec::new();
    let r = references(d.path(), &mut w);
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)));
    let vars: Vec<String> = ok(r).into_iter().map(|r| r.var).collect();
    assert_eq!(vars, ["OK"]);
    assert!(w.len() == 1 && w[0].contains("locked"), "{w:?}");
}

#[test]
fn an_unreadable_file_is_an_error_naming_it() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    put(d.path(), ".env.example", "A=1\n");
    let f = d.path().join(".env.example");
    ok(fs::set_permissions(&f, fs::Permissions::from_mode(0o000)));
    let r = references(d.path(), &mut Vec::new());
    ok(fs::set_permissions(&f, fs::Permissions::from_mode(0o644)));
    let why = r.err().map(|e| format!("{e:#}")).unwrap_or_default();
    assert!(why.contains(".env.example"), "{why}");
}

#[test]
fn a_missing_root_is_an_error() {
    let d = ok(tempfile::tempdir());
    assert!(references(&d.path().join("nope"), &mut Vec::new()).is_err());
}
