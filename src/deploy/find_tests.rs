use super::*;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn touch(root: &Path, rel: &str) {
    let p = root.join(rel);
    if let Some(parent) = p.parent() {
        ok(fs::create_dir_all(parent));
    }
    ok(fs::write(p, "x"));
}

#[test]
fn every_target_finds_its_artifact_at_its_documented_place() {
    let cases: [(Target, &str); 6] = [
        (Target::Ios, "build/ios/ipa/App.ipa"),
        (Target::Android, "build/app/outputs/bundle/release/app-release.aab"),
        (Target::Mas, "build/macos/pkg/App.pkg"),
        (Target::Homebrew, "build/macos/Build/Products/Release/App.app/Contents/x"),
        (Target::Windows, "build/windows/x64/runner/Release/app.exe"),
        (Target::Linux, "build/linux/x64/release/bundle/app"),
    ];
    for (target, rel) in cases {
        let d = ok(tempfile::tempdir());
        assert_eq!(artifact(target, d.path()), None, "{target:?} before the build");
        touch(d.path(), rel);
        let got = artifact(target, d.path());
        let want = match target {
            Target::Windows => "build/windows/x64/runner/Release",
            Target::Linux => "build/linux/x64/release/bundle",
            Target::Homebrew => "build/macos/Build/Products/Release/App.app",
            _ => rel,
        };
        assert_eq!(got, Some(d.path().join(want)), "{target:?}");
    }
}

#[test]
fn web_has_no_artifact_and_other_extensions_do_not_match() {
    let d = ok(tempfile::tempdir());
    touch(d.path(), "build/web/index.html");
    touch(d.path(), "build/ios/ipa/readme.txt");
    assert_eq!(artifact(Target::Web, d.path()), None);
    assert_eq!(artifact(Target::Ios, d.path()), None);
}

#[test]
fn the_first_by_name_is_chosen_at_every_level() {
    let d = ok(tempfile::tempdir());
    touch(d.path(), "build/ios/ipa/b.ipa");
    touch(d.path(), "build/ios/ipa/a.ipa");
    assert_eq!(artifact(Target::Ios, d.path()), Some(d.path().join("build/ios/ipa/a.ipa")));
    touch(d.path(), "build/windows/arm64/runner/Release/x");
    touch(d.path(), "build/windows/x64/runner/Release/x");
    assert_eq!(artifact(Target::Windows, d.path()), Some(d.path().join("build/windows/arm64/runner/Release")));
}

#[test]
fn a_wildcard_level_without_the_rest_of_the_path_does_not_match() {
    let d = ok(tempfile::tempdir());
    touch(d.path(), "build/windows/x64/other/file");
    touch(d.path(), "build/windows/arm64/runner/Release/x");
    assert_eq!(artifact(Target::Windows, d.path()), Some(d.path().join("build/windows/arm64/runner/Release")));
    let e = ok(tempfile::tempdir());
    touch(e.path(), "build/windows/x64/other/file");
    assert_eq!(artifact(Target::Windows, e.path()), None);
}

#[test]
fn deep_finds_a_file_by_extension_anywhere_under_the_folder() {
    let d = ok(tempfile::tempdir());
    assert_eq!(deep(&d.path().join("gone"), "snap"), None);
    touch(d.path(), "a/b/app.snap");
    touch(d.path(), "a/notes.txt");
    assert_eq!(deep(d.path(), "snap"), Some(d.path().join("a/b/app.snap")));
    assert_eq!(deep(d.path(), "msix"), None);
}

#[test]
fn deep_prefers_a_file_of_the_folder_over_a_nested_one_and_ignores_directories_named_like_it() {
    let d = ok(tempfile::tempdir());
    touch(d.path(), "z.snap");
    touch(d.path(), "a/nested.snap");
    assert_eq!(deep(d.path(), "snap"), Some(d.path().join("z.snap")));
    let e = ok(tempfile::tempdir());
    ok(fs::create_dir_all(e.path().join("dir.snap")));
    assert_eq!(deep(e.path(), "snap"), None);
}
