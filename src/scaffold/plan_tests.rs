use super::*;
use tempfile::TempDir;

fn put(root: &Path, rel: &str) {
    let p = root.join(rel);
    let dir = p.parent().unwrap_or(root);
    fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
    fs::write(&p, rel).unwrap_or_else(|e| panic!("{e}"));
}

fn tree() -> TempDir {
    let d = tempfile::tempdir().map_err(|e| e.to_string()).unwrap_or_else(|e| panic!("{e}"));
    for f in [
        "common/README.md.tmpl",
        "common/.gitignore",
        "app/python/.gitignore",
        "app/python/pyproject.toml.tmpl",
        "app/python/skeleton/src/{{project_name}}/main.py.tmpl",
        "workspace/ws.code-workspace",
        "doc/tex/skeleton/chapters/.gitkeep",
    ] {
        put(d.path(), f);
    }
    d
}

fn dsts(entries: &[Entry]) -> Vec<String> {
    let mut v: Vec<_> = entries.iter().map(|e| e.dst.display().to_string()).collect();
    v.sort();
    v
}

#[test]
fn known_types_walk_two_levels_and_keep_leaves_bare() {
    let d = tree();
    let t = known_types(d.path()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(t, ["app/python", "doc/tex", "workspace"]);
}

#[test]
fn common_is_never_a_type() {
    let d = tree();
    let t = known_types(d.path()).unwrap_or_else(|e| panic!("{e}"));
    assert!(!t.iter().any(|t| t.starts_with("common")));
}

#[test]
fn missing_root_has_no_types() {
    let t = known_types(Path::new("/nonexistent/templates")).unwrap_or_else(|e| panic!("{e}"));
    assert!(t.is_empty());
}

#[test]
fn layers_merge_with_tmpl_stripped_and_type_winning() {
    let d = tree();
    let p = plan(d.path(), "app/python").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        dsts(&p),
        [".gitignore", "README.md", "pyproject.toml", "src/{{project_name}}/main.py"]
    );
    let ignore = p.iter().find(|e| e.dst == Path::new(".gitignore"));
    assert!(ignore.is_some_and(|e| e.src.starts_with(d.path().join("app/python")) && !e.template));
}

#[test]
fn template_flag_follows_the_suffix() {
    let d = tree();
    let p = plan(d.path(), "app/python").unwrap_or_else(|e| panic!("{e}"));
    let flags: Vec<_> = p.iter().map(|e| (e.dst.display().to_string(), e.template)).collect();
    assert!(flags.contains(&("README.md".into(), true)));
    assert!(flags.contains(&(".gitignore".into(), false)));
}

#[test]
fn skeleton_is_flattened_into_the_target() {
    let d = tree();
    let p = plan(d.path(), "doc/tex").unwrap_or_else(|e| panic!("{e}"));
    assert!(dsts(&p).contains(&"chapters/.gitkeep".to_string()));
    assert!(!dsts(&p).iter().any(|f| f.starts_with("skeleton")));
}

#[test]
fn leaf_kind_plans_without_a_language() {
    let d = tree();
    let p = plan(d.path(), "workspace").unwrap_or_else(|e| panic!("{e}"));
    assert!(dsts(&p).contains(&"ws.code-workspace".to_string()));
}

#[test]
fn unknown_type_fails_loudly() {
    let d = tree();
    let e = plan(d.path(), "app/cobol").err().map(|e| e.to_string());
    assert_eq!(e.as_deref(), Some("Unknown project type: app/cobol"));
    let e = plan(d.path(), "common").err().map(|e| e.to_string());
    assert_eq!(e.as_deref(), Some("Unknown project type: common"));
}

#[test]
fn bare_kind_that_is_not_a_leaf_is_unknown() {
    let d = tree();
    assert!(plan(d.path(), "app").is_err());
}
