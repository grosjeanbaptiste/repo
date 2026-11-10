use super::*;
use crate::scaffold::{init::init, plan::layered};

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn vars() -> Vars {
    [("project_name", "p"), ("user", "Ann")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

struct Fixture {
    _t: tempfile::TempDir,
    target: tempfile::TempDir,
    plan: Vec<Entry>,
}

fn fixture() -> Fixture {
    let t = ok(tempfile::tempdir());
    let r = t.path();
    ok(fs::create_dir_all(r.join("common")));
    ok(fs::create_dir_all(r.join("app/x/skeleton")));
    ok(fs::write(r.join("common/AGENTS.md.tmpl"), "hi {{user}}\n"));
    ok(symlink("AGENTS.md.tmpl", r.join("common/CLAUDE.md.tmpl")));
    ok(fs::write(r.join("app/x/skeleton/seed.txt"), "seed\n"));
    let plan = ok(layered(r, "app/x", true));
    Fixture { _t: t, target: ok(tempfile::tempdir()), plan }
}

fn lines(f: &Fixture, mode: Mode) -> Vec<String> {
    let mut l = ok(sync(f.target.path(), &f.plan, &vars(), mode));
    l.sort();
    l
}

#[test]
fn check_reports_missing_then_ok_then_drifted() {
    let f = fixture();
    assert_eq!(lines(&f, Mode::Check), ["  - missing  AGENTS.md", "  - missing  CLAUDE.md", "  - missing  seed.txt"]);
    ok(init(f.target.path(), &f.plan, &vars()));
    assert_eq!(lines(&f, Mode::Check), ["  = ok       AGENTS.md", "  = ok       CLAUDE.md", "  = ok       seed.txt"]);
    ok(fs::write(f.target.path().join("AGENTS.md"), "edited"));
    assert!(lines(&f, Mode::Check).contains(&"  ~ drifted  AGENTS.md".to_string()));
}

#[test]
fn check_describes_symlink_drift() {
    let f = fixture();
    ok(init(f.target.path(), &f.plan, &vars()));
    ok(fs::remove_file(f.target.path().join("CLAUDE.md")));
    ok(symlink("other", f.target.path().join("CLAUDE.md")));
    assert!(lines(&f, Mode::Check).contains(&"  ~ drifted  CLAUDE.md (symlink target: other, expected: AGENTS.md)".to_string()));
    ok(fs::remove_file(f.target.path().join("CLAUDE.md")));
    ok(fs::write(f.target.path().join("CLAUDE.md"), "x"));
    assert!(lines(&f, Mode::Check).contains(&"  ~ drifted  CLAUDE.md (not a symlink, expected → AGENTS.md)".to_string()));
}

#[test]
fn apply_creates_missing_and_skips_edited_files() {
    let f = fixture();
    ok(fs::write(f.target.path().join("AGENTS.md"), "mine"));
    let l = lines(&f, Mode::Apply);
    assert!(l.contains(&"  ! exists (skipped, use --force)  AGENTS.md".to_string()));
    assert!(l.contains(&"  + wrote      seed.txt".to_string()));
    assert!(l.contains(&"  + linked     CLAUDE.md -> AGENTS.md".to_string()));
    assert_eq!(ok(fs::read_to_string(f.target.path().join("AGENTS.md"))), "mine");
    assert!(lines(&f, Mode::Apply).iter().any(|l| l == "  ! exists (skipped, use --force)  AGENTS.md"));
}

#[test]
fn apply_twice_on_a_clean_project_changes_nothing() {
    let f = fixture();
    ok(init(f.target.path(), &f.plan, &vars()));
    assert!(lines(&f, Mode::Apply).iter().all(|l| l.starts_with("  = unchanged")));
}

#[test]
fn force_overwrites_files_and_links() {
    let f = fixture();
    ok(fs::write(f.target.path().join("AGENTS.md"), "mine"));
    ok(fs::write(f.target.path().join("CLAUDE.md"), "mine"));
    let l = lines(&f, Mode::Force);
    assert!(l.contains(&"  + wrote      AGENTS.md".to_string()));
    assert!(l.contains(&"  + linked     CLAUDE.md -> AGENTS.md".to_string()));
    assert_eq!(ok(fs::read_to_string(f.target.path().join("AGENTS.md"))), "hi Ann\n");
}

#[test]
fn diff_writes_nothing_and_shows_the_change() {
    let f = fixture();
    ok(fs::write(f.target.path().join("AGENTS.md"), "old\n"));
    let l = lines(&f, Mode::Diff);
    assert!(l.contains(&"  + would create  seed.txt".to_string()));
    assert!(l.contains(&"  + would create symlink  CLAUDE.md -> AGENTS.md".to_string()));
    let upd = l.iter().find(|l| l.starts_with("  ~ would update  AGENTS.md (diff below)"));
    assert!(upd.is_some_and(|u| u.contains("-old") && u.contains("+hi Ann")));
    assert!(!f.target.path().join("seed.txt").exists());
    assert_eq!(ok(fs::read_to_string(f.target.path().join("AGENTS.md"))), "old\n");
}

#[test]
fn nested_destinations_are_created() {
    let t = ok(tempfile::tempdir());
    ok(fs::create_dir_all(t.path().join("common/a/b")));
    ok(fs::write(t.path().join("common/a/b/c.txt"), "c"));
    let plan = ok(layered(t.path(), "none", false));
    let target = ok(tempfile::tempdir());
    ok(sync(target.path(), &plan, &vars(), Mode::Apply));
    assert!(target.path().join("a/b/c.txt").exists());
}

#[test]
fn diff_that_cannot_run_fails_loudly() {
    let f = fixture();
    ok(fs::create_dir(f.target.path().join("seed.txt"))); // a directory where a file belongs
    let e = sync(f.target.path(), &f.plan, &vars(), Mode::Diff).err().map(|e| e.to_string());
    assert!(e.is_some_and(|m| m.contains("diff -u")));
}

#[test]
fn project_name_in_the_destination_is_interpolated() {
    let t = ok(tempfile::tempdir());
    ok(fs::create_dir_all(t.path().join("common/{{project_name}}")));
    ok(fs::write(t.path().join("common/{{project_name}}/f.txt"), "f"));
    let plan = ok(layered(t.path(), "none", false));
    let target = ok(tempfile::tempdir());
    let l = ok(sync(target.path(), &plan, &vars(), Mode::Apply));
    assert_eq!(l, ["  + wrote      p/f.txt"]);
}

#[test]
fn a_regular_file_where_a_link_belongs_is_replaced_only_when_asked() {
    let f = fixture();
    ok(fs::write(f.target.path().join("CLAUDE.md"), "x"));
    assert!(lines(&f, Mode::Diff).contains(&"  ~ would replace with symlink  CLAUDE.md -> AGENTS.md".to_string()));
    assert!(lines(&f, Mode::Apply).contains(&"  ! exists (skipped, use --force)  CLAUDE.md".to_string()));
    assert_eq!(ok(fs::read_to_string(f.target.path().join("CLAUDE.md"))), "x");
}

#[test]
fn a_dangling_link_where_a_file_belongs_counts_as_existing() {
    let f = fixture();
    ok(symlink("/nonexistent", f.target.path().join("seed.txt")));
    assert!(lines(&f, Mode::Check).contains(&"  ~ drifted  seed.txt".to_string()));
    assert!(lines(&f, Mode::Apply).contains(&"  ! exists (skipped, use --force)  seed.txt".to_string()));
}

#[test]
fn an_unreadable_existing_path_is_drifted_not_missing() {
    let f = fixture();
    ok(fs::create_dir(f.target.path().join("AGENTS.md")));
    assert!(lines(&f, Mode::Check).contains(&"  ~ drifted  AGENTS.md".to_string()));
}
