use super::*;
use crate::deploy::fake::{Fake, message};
use crate::deploy::target::Env;
use crate::deploy::vars::Vars;
use std::fs;
use std::path::Path;

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

struct Out {
    result: Result<()>,
    tool: Fake,
    out: String,
}

const VARS: [(&str, &str); 8] = [
    ("ASC_KEY_ID", "K"),
    ("ASC_ISSUER_ID", "I"),
    ("ASC_BUNDLE_ID", "b"),
    ("STORES_PYTHON", "/py"),
    ("STORES_SCRIPTS_DIR", "/s"),
    ("PLAY_PACKAGE_NAME", "com.x"),
    ("BREW_APP_URL", "https://h/a.zip"),
    ("SNAPCRAFT_STORE_CREDENTIALS", "tok"),
];

fn go(root: &Path, env: Env, target: Target, dry_run: bool, extra: &[(&str, &str)], tool: Fake) -> Out {
    let home = root.join("home");
    let vars = Vars::of(&[&VARS[..], extra].concat(), &home);
    let (mut out, mut err) = (vec![], vec![]);
    let result = {
        let mut c = Ctx { env, root, vars: &vars, tool: &tool, out: &mut out, err: &mut err };
        deploy(&mut c, target, dry_run)
    };
    Out { result, tool, out: String::from_utf8_lossy(&out).into_owned() }
}

fn root() -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join("pubspec.yaml"), "name: a\nversion: 1.0.0+1\n"));
    d
}

fn lines(o: &Out) -> Vec<String> {
    o.tool.lines()
}

#[test]
fn ios_builds_the_ipa_signed_for_the_store_then_uploads_and_submits() {
    let d = root();
    touch(d.path(), "ios/ExportOptions.plist");
    touch(d.path(), "build/ios/ipa/App.ipa");
    let o = go(d.path(), Env::Prod, Target::Ios, false, &[], Fake::default());
    assert!(o.result.is_ok(), "{}", message(&o.result));
    let ipa = d.path().join("build/ios/ipa/App.ipa");
    assert_eq!(lines(&o)[0], "flutter build ipa --release --export-options-plist ios/ExportOptions.plist");
    assert_eq!(o.tool.runs()[0].cwd.as_deref(), Some(d.path()));
    assert_eq!(lines(&o)[1], format!("xcrun altool --upload-app --type ios --file {} --apiKey K --apiIssuer I", ipa.display()));
    assert_eq!(lines(&o)[2], "/py /s/asc_upload.py --submit --platform IOS");
    assert_eq!(lines(&o).len(), 3);
}

#[test]
fn ios_without_export_options_builds_plain_and_test_stops_at_testflight() {
    let d = root();
    touch(d.path(), "build/ios/ipa/App.ipa");
    let o = go(d.path(), Env::Test, Target::Ios, false, &[], Fake::default());
    assert_eq!(lines(&o)[0], "flutter build ipa --release");
    assert_eq!(lines(&o).len(), 2);
}

#[test]
fn every_other_target_builds_its_flutter_subcommand_in_the_project() {
    for (target, sub) in [(Target::Android, "appbundle"), (Target::Homebrew, "macos"), (Target::Windows, "windows"), (Target::Linux, "linux")] {
        let d = root();
        let o = go(d.path(), Env::Test, target, false, &[], Fake::default());
        let runs = o.tool.runs();
        assert_eq!(runs[0].args, ["build", sub, "--release"], "{target:?}");
        assert_eq!(runs[0].cwd.as_deref(), Some(d.path()));
    }
}

#[test]
fn each_target_ends_on_its_own_channel() {
    for (target, artifact, last) in [
        (Target::Android, "build/app/outputs/bundle/release/a.aab", "/py"),
        (Target::Homebrew, "", "/py"),
        (Target::Windows, "build/win/app.msix", "msstore"),
        (Target::Linux, "build/snap/a.snap", "snapcraft"),
    ] {
        let d = root();
        ok(fs::create_dir_all(d.path().join("stores/brew/Casks")));
        if !artifact.is_empty() {
            touch(d.path(), artifact);
        }
        let sa = d.path().join("sa.json");
        ok(fs::write(&sa, "{}"));
        let o = go(d.path(), Env::Test, target, false, &[("PLAY_SERVICE_ACCOUNT_PATH", sa.to_str().unwrap_or_default())], Fake::default());
        assert!(o.result.is_ok(), "{target:?}: {}", message(&o.result));
        assert_eq!(o.tool.runs().last().map(|r| r.prog.clone()).as_deref(), Some(last), "{target:?}");
        assert_eq!(o.tool.runs().len(), 2, "{target:?}: one build, one publish");
    }
}

#[test]
fn a_failed_build_publishes_nothing() {
    let d = root();
    touch(d.path(), "build/ios/ipa/App.ipa");
    let o = go(d.path(), Env::Prod, Target::Ios, false, &[], Fake { failing: vec!["flutter".into()], ..Fake::default() });
    assert!(o.result.is_err());
    assert_eq!(o.tool.runs().len(), 1);
}

#[test]
fn a_build_that_left_no_artifact_is_named() {
    let d = root();
    let o = go(d.path(), Env::Prod, Target::Ios, false, &[], Fake::default());
    assert_eq!(message(&o.result), format!("no ios artifact under {}/build.", d.path().display()));
}

#[test]
fn web_has_no_build_recipe_here() {
    let d = root();
    let o = go(d.path(), Env::Prod, Target::Web, false, &[], Fake::default());
    assert_eq!(message(&o.result), "no build recipe for 'web'");
    assert!(o.tool.runs().is_empty());
}

#[test]
fn dry_run_prints_the_plan_and_runs_nothing() {
    let d = root();
    let r = d.path().display();
    let o = go(d.path(), Env::Prod, Target::Android, true, &[], Fake::default());
    assert_eq!(o.out, format!("flutter build appbundle --release (cwd {r}) then publish android to prod\n"));
    assert!(o.tool.runs().is_empty());
    touch(d.path(), "ios/ExportOptions.plist");
    let o = go(d.path(), Env::Test, Target::Ios, true, &[], Fake::default());
    assert_eq!(o.out, format!("flutter build ipa --release --export-options-plist ios/ExportOptions.plist (cwd {r}) then publish ios to test\n"));
    let o = go(d.path(), Env::Test, Target::Mas, true, &[], Fake::default());
    assert_eq!(o.out, format!("macOS App Store: flutter config-only + xcodebuild archive + export signed .pkg (cwd {r}) then upload to test\n"));
    assert!(o.tool.runs().is_empty());
}

#[test]
fn mas_needs_the_manual_signing_export_options() {
    let d = root();
    let o = go(d.path(), Env::Prod, Target::Mas, false, &[], Fake::default());
    assert_eq!(message(&o.result), format!("missing macos/ExportOptions.plist under {} (manual app-store signing).", d.path().display()));
    assert!(o.tool.runs().is_empty());
}

#[test]
fn mas_archives_exports_a_signed_pkg_clears_stale_output_and_uploads_it() {
    let d = root();
    touch(d.path(), "macos/ExportOptions.plist");
    touch(d.path(), "build/macos/archive/Runner.xcarchive/stale");
    touch(d.path(), "build/macos/pkg/Old.pkg");
    let r = d.path().display().to_string();
    let o = go(d.path(), Env::Test, Target::Mas, false, &[], Fake::default());
    let archive = format!("{r}/build/macos/archive/Runner.xcarchive");
    let pkgdir = format!("{r}/build/macos/pkg");
    assert_eq!(message(&o.result), format!("export produced no .pkg under {pkgdir}."), "the fake exports nothing");
    assert!(!d.path().join("build/macos/archive/Runner.xcarchive/stale").exists());
    assert!(!d.path().join("build/macos/pkg/Old.pkg").exists());
    assert_eq!(
        lines(&o),
        vec![
            "flutter build macos --release --config-only".to_string(),
            format!("xcodebuild -workspace macos/Runner.xcworkspace -scheme Runner -configuration Release -archivePath {archive} archive -allowProvisioningUpdates -quiet"),
            format!("xcodebuild -exportArchive -archivePath {archive} -exportPath {pkgdir} -exportOptionsPlist {r}/macos/ExportOptions.plist -allowProvisioningUpdates"),
        ]
    );
    assert!(o.tool.runs().iter().all(|x| x.cwd.as_deref() == Some(d.path())));
}

#[test]
fn mas_uploads_the_pkg_as_macos_once_it_exists() {
    let d = root();
    touch(d.path(), "macos/ExportOptions.plist");
    let pkg = d.path().join("build/macos/pkg/App.pkg");
    let tool = Fake { on_run: vec![("xcodebuild".into(), "-exportArchive".into(), pkg.clone())], ..Fake::default() };
    let o = go(d.path(), Env::Test, Target::Mas, false, &[], tool);
    assert!(o.result.is_ok(), "{}", message(&o.result));
    assert_eq!(lines(&o).last().cloned(), Some(format!("xcrun altool --upload-app --type macos --file {} --apiKey K --apiIssuer I", pkg.display())));
}

#[test]
fn a_failing_archive_or_export_stops_mas_before_the_upload() {
    for failing in ["flutter", "xcodebuild"] {
        let d = root();
        touch(d.path(), "macos/ExportOptions.plist");
        let o = go(d.path(), Env::Prod, Target::Mas, false, &[], Fake { failing: vec![failing.into()], ..Fake::default() });
        assert!(o.result.is_err(), "{failing}");
        assert!(o.tool.runs().iter().all(|r| r.prog != "xcrun"));
    }
}

#[test]
fn stale_output_that_cannot_be_cleared_stops_mas_loudly() {
    use std::os::unix::fs::PermissionsExt;
    let d = root();
    touch(d.path(), "macos/ExportOptions.plist");
    touch(d.path(), "build/macos/archive/Runner.xcarchive/stale");
    let locked = d.path().join("build/macos/archive/Runner.xcarchive");
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)));
    let o = go(d.path(), Env::Test, Target::Mas, false, &[], Fake::default());
    ok(fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)));
    assert!(message(&o.result).starts_with("clearing "), "{}", message(&o.result));
    assert!(o.tool.runs().iter().all(|r| r.prog != "xcodebuild"));
}
