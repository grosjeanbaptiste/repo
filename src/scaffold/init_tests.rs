use super::*;
use crate::scaffold::plan::plan;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn vars() -> Vars {
    [("project_name", "ifdck"), ("user", "Ann"), ("email", "a@b.c"), ("year", "2026")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn templates() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    let r = d.path();
    ok(fs::create_dir_all(r.join("common")));
    ok(fs::write(r.join("common/AGENTS.md.tmpl"), "agent {{user}} <{{email}}>"));
    ok(symlink("AGENTS.md.tmpl", r.join("common/CLAUDE.md.tmpl")));
    ok(fs::write(r.join("common/plain.txt"), "{{user}} stays"));
    ok(fs::create_dir_all(r.join("app/rust/skeleton/src/{{project_name}}")));
    ok(fs::write(r.join("app/rust/skeleton/src/{{project_name}}/lib.rs.tmpl"), "// {{project_name}} {{year}}"));
    ok(fs::write(r.join("app/rust/run.sh"), "#!/bin/sh\n"));
    ok(fs::set_permissions(r.join("app/rust/run.sh"), fs::Permissions::from_mode(0o755)));
    d
}

fn scaffold() -> (tempfile::TempDir, std::path::PathBuf) {
    let t = templates();
    let out = ok(tempfile::tempdir());
    let target = out.path().join("ifdck");
    ok(fs::create_dir(&target));
    ok(init(&target, &ok(plan(t.path(), "app/rust")), &vars()));
    std::mem::forget(t); // keep template dir alive for symlink targets; the OS reaps /tmp
    (out, target)
}

#[test]
fn templates_are_rendered_and_plain_files_copied_verbatim() {
    let (_o, t) = scaffold();
    assert_eq!(ok(fs::read_to_string(t.join("AGENTS.md"))), "agent Ann <a@b.c>");
    assert_eq!(ok(fs::read_to_string(t.join("plain.txt"))), "{{user}} stays");
}

#[test]
fn template_symlink_becomes_a_symlink_to_the_rendered_sibling() {
    let (_o, t) = scaffold();
    assert_eq!(ok(fs::read_link(t.join("CLAUDE.md"))).to_string_lossy(), "AGENTS.md");
    assert_eq!(ok(fs::read_to_string(t.join("CLAUDE.md"))), "agent Ann <a@b.c>");
}

#[test]
fn project_name_in_a_path_names_the_directory() {
    let (_o, t) = scaffold();
    assert_eq!(ok(fs::read_to_string(t.join("src/ifdck/lib.rs"))), "// ifdck 2026");
}

#[test]
fn exec_bit_is_kept() {
    let (_o, t) = scaffold();
    assert_eq!(ok(fs::metadata(t.join("run.sh"))).permissions().mode() & 0o111, 0o111);
    assert_eq!(ok(fs::metadata(t.join("plain.txt"))).permissions().mode() & 0o111, 0);
}

#[test]
fn init_overwrites_existing_files_and_links() {
    let t = templates();
    let out = ok(tempfile::tempdir());
    ok(fs::write(out.path().join("AGENTS.md"), "old"));
    ok(fs::write(out.path().join("CLAUDE.md"), "old file"));
    ok(init(out.path(), &ok(plan(t.path(), "app/rust")), &vars()));
    assert_eq!(ok(fs::read_to_string(out.path().join("AGENTS.md"))), "agent Ann <a@b.c>");
    assert!(ok(fs::symlink_metadata(out.path().join("CLAUDE.md"))).file_type().is_symlink());
}

#[test]
fn init_twice_is_idempotent() {
    let t = templates();
    let out = ok(tempfile::tempdir());
    let p = ok(plan(t.path(), "app/rust"));
    ok(init(out.path(), &p, &vars()));
    ok(init(out.path(), &p, &vars()));
    assert_eq!(ok(fs::read_link(out.path().join("CLAUDE.md"))).to_string_lossy(), "AGENTS.md");
}

#[test]
fn unreadable_template_fails_loudly() {
    let t = templates();
    let out = ok(tempfile::tempdir());
    let p = ok(plan(t.path(), "app/rust"));
    ok(fs::write(t.path().join("common/AGENTS.md.tmpl"), [0xff, 0xfe]));
    let e = init(out.path(), &p, &vars()).err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("reading template")));
}
