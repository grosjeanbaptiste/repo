use super::*;
use crate::deploy::fake::Fake;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn with<R>(pairs: &[(&str, &str)], f: impl FnOnce(&Vars) -> R) -> R {
    f(&Vars::of(pairs, Path::new("/home/u")))
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(parent) = p.parent() {
        ok(fs::create_dir_all(parent));
    }
    ok(fs::write(p, text));
}

const DEFAULT_SCRIPTS: &str = "/home/u/dotfiles/project-templates/mobile/native/stores/scripts";

#[test]
fn the_scripts_dir_follows_its_overrides_then_the_dotfiles_checkout() {
    assert_eq!(with(&[], scripts_dir), PathBuf::from(DEFAULT_SCRIPTS));
    assert_eq!(
        with(&[("DOTFILES_REPO_ROOT", "/d")], scripts_dir),
        PathBuf::from("/d/project-templates/mobile/native/stores/scripts")
    );
    assert_eq!(
        with(&[("DOTFILES_REPO_ROOT", "/d"), ("STORES_SCRIPTS_DIR", "/s")], scripts_dir),
        PathBuf::from("/s")
    );
    assert_eq!(with(&[("STORES_SCRIPTS_DIR", ""), ("DOTFILES_REPO_ROOT", "")], scripts_dir), PathBuf::from(DEFAULT_SCRIPTS));
}

fn python_in(home: &Path, pairs: &[(&str, &str)], tool: &Fake) -> (Result<PathBuf>, String) {
    let mut err = vec![];
    let r = python(&Vars::of(pairs, home), tool, &mut err);
    (r, String::from_utf8_lossy(&err).into_owned())
}

fn venv_python(home: &Path) -> PathBuf {
    home.join(".cache/dotfiles/stores-venv/bin/python")
}

#[test]
fn stores_python_is_used_as_is() {
    let h = ok(tempfile::tempdir());
    let t = Fake::default();
    let (r, e) = python_in(h.path(), &[("STORES_PYTHON", "/opt/py")], &t);
    assert_eq!(ok(r), PathBuf::from("/opt/py"));
    assert!(e.is_empty() && t.runs().is_empty());
}

#[test]
fn an_executable_managed_venv_is_reused_without_provisioning() {
    let h = ok(tempfile::tempdir());
    let py = venv_python(h.path());
    put(h.path(), ".cache/dotfiles/stores-venv/bin/python", "#!/bin/sh\n");
    ok(fs::set_permissions(&py, fs::Permissions::from_mode(0o755)));
    let t = Fake::default();
    let (r, e) = python_in(h.path(), &[], &t);
    assert_eq!(ok(r), py);
    assert!(e.is_empty() && t.runs().is_empty());
}

#[test]
fn a_missing_or_non_executable_venv_is_provisioned_and_said_on_stderr() {
    for executable_file in [false, true] {
        let h = ok(tempfile::tempdir());
        if executable_file {
            put(h.path(), ".cache/dotfiles/stores-venv/bin/python", "x");
            ok(fs::set_permissions(venv_python(h.path()), fs::Permissions::from_mode(0o644)));
        }
        let t = Fake::default();
        let (r, e) = python_in(h.path(), &[("STORES_SCRIPTS_DIR", "/s")], &t);
        let venv = h.path().join(".cache/dotfiles/stores-venv");
        assert_eq!(ok(r), venv_python(h.path()));
        assert_eq!(e, format!("deploy: provisioning stores venv at {} …\n", venv.display()));
        assert_eq!(
            t.lines(),
            vec![
                format!("python3 -m venv {}", venv.display()),
                format!("{} -m pip install -q -r /s/requirements.txt", venv_python(h.path()).display()),
            ]
        );
    }
}

#[test]
fn a_failing_venv_or_pip_is_an_error() {
    let h = ok(tempfile::tempdir());
    for failing in ["python3", venv_python(h.path()).to_str().unwrap_or_default()] {
        let t = Fake { failing: vec![failing.to_string()], ..Fake::default() };
        assert!(python_in(h.path(), &[], &t).0.is_err(), "{failing}");
    }
}

fn pkg(root: &Path, pairs: &[(&str, &str)]) -> Option<String> {
    ok(with(pairs, |v| android_package(v, root)))
}

#[test]
fn the_package_env_var_wins_over_gradle() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "android/app/build.gradle", "applicationId \"com.gradle\"\n");
    assert_eq!(pkg(d.path(), &[("PLAY_PACKAGE_NAME", "com.env")]), Some("com.env".into()));
    assert_eq!(pkg(d.path(), &[("PLAY_PACKAGE_NAME", "")]), Some("com.gradle".into()));
}

#[test]
fn the_application_id_is_read_in_every_gradle_spelling() {
    for (file, line) in [
        ("build.gradle", "    applicationId \"com.a.b\""),
        ("build.gradle", "    applicationId = \"com.a.b\""),
        ("build.gradle.kts", "    applicationId = \"com.a.b\""),
        ("build.gradle", "applicationId   =   \"com.a.b\" // note"),
    ] {
        let d = ok(tempfile::tempdir());
        put(d.path(), &format!("android/app/{file}"), &format!("android {{\n{line}\n}}\n"));
        assert_eq!(pkg(d.path(), &[]), Some("com.a.b".into()), "{file}: {line}");
    }
}

#[test]
fn without_gradle_or_application_id_there_is_no_package() {
    let d = ok(tempfile::tempdir());
    assert_eq!(pkg(d.path(), &[]), None);
    put(d.path(), "android/app/build.gradle", "applicationId com.unquoted\nnamespace \"x\"\n");
    assert_eq!(pkg(d.path(), &[]), None);
}

#[test]
fn the_first_application_id_of_the_first_gradle_file_wins() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "android/app/build.gradle.kts", "applicationId = \"com.kts\"\n");
    put(d.path(), "android/app/build.gradle", "applicationId \"com.first\"\napplicationId \"com.second\"\n");
    assert_eq!(pkg(d.path(), &[]), Some("com.first".into()));
}

#[test]
fn the_pubspec_version_loses_its_build_number() {
    for (text, want) in [
        ("name: a\nversion: 1.2.3+4\n", Some("1.2.3")),
        ("version:   2.0.0\n", Some("2.0.0")),
        ("version: 3.1.0+7+8\n", Some("3.1.0")),
        ("name: a\n  version: 9.9.9\n", None),
        ("name: a\n", None),
        ("version:\n", None),
        ("version: 1.0.0\nversion: 2.0.0\n", Some("1.0.0")),
    ] {
        let d = ok(tempfile::tempdir());
        put(d.path(), "pubspec.yaml", text);
        assert_eq!(ok(pubspec_version(d.path())).as_deref(), want, "{text:?}");
    }
    let e = ok(tempfile::tempdir());
    assert_eq!(ok(pubspec_version(e.path())), None);
}

#[test]
fn an_unreadable_pubspec_or_android_folder_is_an_error_not_an_absent_value() {
    let d = ok(tempfile::tempdir());
    put(d.path(), "pubspec.yaml", "version: 1.0.0\n");
    ok(fs::set_permissions(d.path().join("pubspec.yaml"), fs::Permissions::from_mode(0o000)));
    assert!(pubspec_version(d.path()).is_err());
    put(d.path(), "android/app/build.gradle", "x");
    ok(fs::set_permissions(d.path().join("android/app"), fs::Permissions::from_mode(0o000)));
    let r = with(&[], |v| android_package(v, d.path()));
    ok(fs::set_permissions(d.path().join("android/app"), fs::Permissions::from_mode(0o755)));
    assert!(r.is_err());
}

#[test]
fn an_application_id_needs_a_separator_and_a_non_empty_value() {
    for line in ["applicationId\"com.x\"", "applicationId \"\"", "applicationId = \"\"", "applicationId"] {
        let d = ok(tempfile::tempdir());
        put(d.path(), "android/app/build.gradle", &format!("{line}\n"));
        assert_eq!(pkg(d.path(), &[]), None, "{line}");
    }
}
