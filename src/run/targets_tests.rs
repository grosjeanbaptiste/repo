use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        ok(fs::create_dir_all(parent));
    }
    ok(fs::write(p, text));
}

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn targets(dir: &Path) -> (Vec<String>, Vec<String>) {
    let mut w = vec![];
    let t = ok(makefile_targets(&dir.join("Makefile"), &mut w));
    (t, w)
}

#[test]
fn kind_priority_is_makefile_then_flutter_then_cargo_then_unknown() {
    let d = ok(tempfile::tempdir());
    assert_eq!(detect(d.path()), Kind::Unknown);
    put(d.path(), "Cargo.toml", "");
    assert_eq!(detect(d.path()), Kind::Cargo);
    put(d.path(), "pubspec.yaml", "");
    assert_eq!(detect(d.path()), Kind::Flutter);
    put(d.path(), "Makefile", "");
    assert_eq!(detect(d.path()), Kind::Makefile);
}

#[test]
fn a_directory_named_makefile_is_not_a_makefile() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("Makefile")));
    assert_eq!(detect(d.path()), Kind::Unknown);
}

#[test]
fn makefile_targets_exclude_special_targets_assignments_and_pattern_rules() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", ".PHONY: all\nVAR := 1\nOTHER ?= 2\n%.o: %.c\nall:\n\tcc\nbuild-x.y_z: all\n\techo\n  indented:\n_priv:\n9bad:\nA=b:c\n");
    assert_eq!(targets(d.path()).0, v(&["all", "build-x.y_z", "_priv"]));
}

#[test]
fn a_target_followed_by_a_colon_at_end_of_line_or_a_space_counts() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "a:\nb: dep\nc::\nd:=x\n");
    assert_eq!(targets(d.path()).0, v(&["a", "b", "c"]));
}

#[test]
fn includes_are_followed_relative_to_the_including_file_and_deduplicated() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "all:\n-include Makefile.dev\ninclude sub/more.mk\nshared:\n");
    put(d.path(), "Makefile.dev", "bootstrap:\nshared:\n");
    put(d.path(), "sub/more.mk", "deep:\n");
    let (t, w) = targets(d.path());
    assert_eq!(t, v(&["all", "shared", "bootstrap", "deep"]));
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn includes_nest_and_each_resolves_against_its_own_directory() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "include sub/a.mk\n");
    put(d.path(), "sub/a.mk", "include b.mk\na:\n");
    put(d.path(), "sub/b.mk", "b:\n");
    assert_eq!(targets(d.path()).0, v(&["a", "b"]));
}

#[test]
fn a_missing_dash_include_is_quiet_but_a_missing_plain_include_warns() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "all:\n-include nope.mk\n");
    assert!(targets(d.path()).1.is_empty());
    put(d.path(), "Makefile", "all:\ninclude nope.mk\n");
    let (t, w) = targets(d.path());
    assert_eq!(t, v(&["all"]));
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("nope.mk") && w[0].contains("include"), "{w:?}");
}

#[test]
fn an_include_cycle_ends_and_warns() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "include a.mk\nm:\n");
    put(d.path(), "a.mk", "include Makefile\na:\n");
    let (t, w) = targets(d.path());
    assert_eq!(t, v(&["m", "a"]));
    assert!(w.iter().any(|x| x.contains("cycle")), "{w:?}");
}

#[test]
fn only_a_single_bare_filename_per_include_is_understood() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "include a.mk b.mk\ninclude $(DIR)/x.mk\ninclude *.mk\nall:\n");
    put(d.path(), "a.mk", "a:\n");
    assert_eq!(targets(d.path()).0, v(&["all"]));
}

#[test]
fn an_unreadable_makefile_is_an_error_not_an_empty_list() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "all:\n");
    ok(fs::set_permissions(d.path().join("Makefile"), fs::Permissions::from_mode(0o000)));
    let mut w = vec![];
    let e = makefile_targets(&d.path().join("Makefile"), &mut w).err().map(|e| format!("{e:#}"));
    assert!(e.is_some_and(|m| m.contains("Makefile")), "must name the file");
}

#[test]
fn melos_scripts_are_the_two_space_indented_names_under_scripts() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "melos.yaml", "name: x\npackages:\n  - apps/*\nscripts:\n  gen:\n    run: melos exec -- build\n  watch:\n    description: w\n  inline: value\n  deep-name_1:   \n    run: x\ncommand:\n  bootstrap:\n");
    assert_eq!(ok(melos_scripts(&d.path().join("melos.yaml"))), v(&["gen", "watch", "deep-name_1"]));
}

#[test]
fn melos_scripts_stop_at_the_next_top_level_key_and_missing_file_is_an_error() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "melos.yaml", "scripts:\n  a:\nother:\n  b:\n");
    assert_eq!(ok(melos_scripts(&d.path().join("melos.yaml"))), v(&["a"]));
    assert!(melos_scripts(&d.path().join("gone.yaml")).is_err());
}

fn list_of(dir: &Path) -> (Vec<String>, Vec<String>) {
    let mut w = vec![];
    let l = ok(list(dir, &mut w));
    (l, w)
}

#[test]
fn flutter_cargo_and_unknown_projects_list_their_fixed_verbs() {
    let d = ok(tempfile::tempdir());
    assert!(list_of(d.path()).0.is_empty());
    put(d.path(), "Cargo.toml", "");
    assert_eq!(list_of(d.path()).0, v(&["run", "build", "test", "check", "clippy"]));
    let f = ok(tempfile::tempdir());
    put(f.path(), "pubspec.yaml", "");
    assert_eq!(list_of(f.path()).0, v(&["run", "test", "build", "clean", "pub-get", "install", "publish"]));
}

#[test]
fn makefile_targets_come_first_then_melos_scripts_without_duplicates() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "run_x:\ngen:\n");
    put(d.path(), "melos.yaml", "scripts:\n  gen:\n    run: x\n  extra:\n    run: y\n");
    assert_eq!(list_of(d.path()).0, v(&["run_x", "gen", "extra"]));
}

#[test]
fn melos_scripts_are_listed_on_top_of_any_base_kind() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Cargo.toml", "");
    put(d.path(), "melos.yaml", "scripts:\n  gen:\n");
    assert_eq!(list_of(d.path()).0, v(&["run", "build", "test", "check", "clippy", "gen"]));
}

#[test]
fn a_store_release_script_adds_publish_once() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "pubspec.yaml", "");
    put(d.path(), "stores/scripts/tf_release.py", "x");
    let l = list_of(d.path()).0;
    assert_eq!(l.iter().filter(|x| *x == "publish").count(), 1);
    let c = ok(tempfile::tempdir());
    put(c.path(), "stores/scripts/tf_release.py", "x");
    assert_eq!(list_of(c.path()).0, v(&["publish"]));
}

#[test]
fn an_executable_device_hook_adds_install_and_a_non_executable_one_warns() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "scripts/device-install.sh", "#!/bin/sh\n");
    let (l, w) = list_of(d.path());
    assert!(l.is_empty() && w.len() == 1 && w[0].contains("not executable"), "{l:?} {w:?}");
    ok(fs::set_permissions(d.path().join("scripts/device-install.sh"), fs::Permissions::from_mode(0o755)));
    let (l, w) = list_of(d.path());
    assert_eq!(l, v(&["install"]));
    assert!(w.is_empty());
}

#[test]
fn unsupported_include_forms_are_skipped_without_a_warning() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "Makefile", "include \ninclude a.mk b.mk\ninclude $(DIR)/x.mk\ninclude *.mk\ninclude a?.mk\ninclude [ab].mk\nall:\n");
    let (t, w) = targets(d.path());
    assert_eq!(t, v(&["all"]));
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn keys_of_a_section_before_scripts_are_not_scripts() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "melos.yaml", "command:\n  bootstrap:\nscripts:\n  a:\n");
    assert_eq!(ok(melos_scripts(&d.path().join("melos.yaml"))), v(&["a"]));
    put(d.path(), "melos.yaml", "command:\n  bootstrap:\n");
    assert!(ok(melos_scripts(&d.path().join("melos.yaml"))).is_empty());
}

#[test]
fn only_plain_names_count_as_melos_scripts() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "melos.yaml", "scripts:\n  :\n  has space:\n  \"q\":\n  ok_1:\n");
    assert_eq!(ok(melos_scripts(&d.path().join("melos.yaml"))), v(&["ok_1"]));
}
