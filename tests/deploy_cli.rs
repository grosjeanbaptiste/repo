//! `repo deploy` end to end: a real project folder and scripted `flutter`, `xcrun`, `wrangler`, `snapcraft`, `git` and uploaders.
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Logs `<name> <args>` and the variables of interest to $STAGE/calls.log; exits with $<NAME>_EXIT.
const TOOL: &str = r#"#!/bin/sh
name=$(basename "$0")
echo "$name $*" >> "$STAGE/calls.log"
[ -z "$PLAY_PACKAGE_NAME$SNAPCRAFT_STORE_CREDENTIALS$STORES_DIR" ] || echo "  env: pkg=$PLAY_PACKAGE_NAME creds=$SNAPCRAFT_STORE_CREDENTIALS stores=$STORES_DIR" >> "$STAGE/calls.log"
code=$(printenv "$(echo "$name" | tr a-z A-Z)_EXIT")
exit "${code:-0}"
"#;

const GIT: &str = "#!/bin/sh\necho feat-branch\n";

struct Fx {
    _d: tempfile::TempDir,
    base: PathBuf,
    project: PathBuf,
}

fn script(path: &Path, text: &str) {
    ok(fs::write(path, text));
    ok(fs::set_permissions(path, fs::Permissions::from_mode(0o755)));
}

fn touch(root: &Path, rel: &str) {
    let p = root.join(rel);
    ok(fs::create_dir_all(ok(p.parent().ok_or("no parent"))));
    ok(fs::write(p, "x"));
}

fn fx() -> Fx {
    let d = ok(tempfile::tempdir());
    let base = ok(d.path().canonicalize());
    let project = base.join("app");
    for p in [&project, &base.join("bin"), &base.join("home")] {
        ok(fs::create_dir_all(p));
    }
    for t in ["flutter", "xcrun", "wrangler", "snapcraft", "msstore", "py"] {
        script(&base.join("bin").join(t), TOOL);
    }
    script(&base.join("bin/git"), GIT);
    script(&base.join("bin/dotfiles"), DOTFILES);
    Fx {
        _d: d,
        base,
        project,
    }
}

const ENV: [(&str, &str); 4] = [
    ("ASC_KEY_ID", "K"),
    ("ASC_ISSUER_ID", "I"),
    ("ASC_BUNDLE_ID", "com.x"),
    ("STORES_SCRIPTS_DIR", "/s"),
];

/// The secret store: only `secret get snapcraft-store-credentials` answers.
const DOTFILES: &str =
    "#!/bin/sh\n[ \"$*\" = \"secret get snapcraft-store-credentials\" ] && echo from-secret\n";

impl Fx {
    fn repo(&self, args: &[&str], stdin: &str, extra: &[(&str, &str)]) -> Output {
        let path = format!("{}:/usr/bin:/bin", self.base.join("bin").display());
        let mut child = ok(Command::new(env!("CARGO_BIN_EXE_repo"))
            .arg("deploy")
            .args(args)
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.base.join("home"))
            .env("STAGE", &self.base)
            .env("STORES_PYTHON", self.base.join("bin/py"))
            .envs(ENV)
            .envs(extra.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn());
        if let Some(mut input) = child.stdin.take() {
            ok(input.write_all(stdin.as_bytes()));
        }
        ok(child.wait_with_output())
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.base.join("calls.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    fn p(&self) -> &str {
        self.project.to_str().unwrap_or_default()
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn an_invalid_env_or_target_exits_1_naming_the_value() {
    let f = fx();
    let o = f.repo(&["staging", "web", f.p()], "", &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("invalid env 'staging' — expected: prod | test"),
        "{}",
        err(&o)
    );
    let o = f.repo(&["prod", "pc", f.p()], "", &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains(
            "invalid target 'pc' — expected: web ios android mas homebrew windows linux | all"
        ),
        "{}",
        err(&o)
    );
    assert!(f.calls().is_empty());
}

#[test]
fn web_dry_run_prints_the_wrangler_command_and_runs_nothing() {
    let f = fx();
    touch(&f.project, "dist/index.html");
    let o = f.repo(
        &["prod", "web", f.p(), "--dry-run", "--project", "site"],
        "",
        &[],
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        out(&o),
        format!(
            "wrangler pages deploy {}/dist --project-name=site --branch=main\n",
            f.project.display()
        )
    );
    assert!(f.calls().is_empty());
}

#[test]
fn web_test_deploys_to_the_git_branch_through_wrangler() {
    let f = fx();
    touch(&f.project, "build/web/index.html");
    let o = f.repo(&["test", "web", f.p(), "--yes"], "", &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        f.calls(),
        vec![format!(
            "wrangler pages deploy {}/build/web --project-name=app --branch=feat-branch",
            f.project.display()
        )]
    );
}

#[test]
fn the_question_is_asked_on_stdin_and_a_no_aborts_with_one() {
    let f = fx();
    touch(&f.project, "dist/index.html");
    let o = f.repo(&["prod", "web", f.p()], "n\n", &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        out(&o).contains(&format!(
            "Deploy 'web' to 'prod' from {}? [y/N] ",
            f.project.display()
        )),
        "{}",
        out(&o)
    );
    assert!(err(&o).contains("Aborted."), "{}", err(&o));
    assert!(f.calls().is_empty());
    let o = f.repo(&["prod", "web", f.p()], "yes\n", &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(f.calls().len(), 1);
}

#[test]
fn ios_test_builds_then_uploads_to_testflight_only() {
    let f = fx();
    touch(&f.project, "build/ios/ipa/App.ipa");
    let o = f.repo(&["test", "ios", f.p(), "-y"], "", &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let ipa = f.project.join("build/ios/ipa/App.ipa");
    assert_eq!(
        f.calls(),
        vec![
            "flutter build ipa --release".to_string(),
            format!(
                "xcrun altool --upload-app --type ios --file {} --apiKey K --apiIssuer I",
                ipa.display()
            ),
        ]
    );
}

#[test]
fn android_reaches_the_uploader_with_package_and_service_account_in_its_environment() {
    let f = fx();
    touch(&f.project, "build/app/outputs/bundle/release/app.aab");
    ok(fs::create_dir_all(f.project.join("android/app")));
    ok(fs::write(
        f.project.join("android/app/build.gradle"),
        "android { defaultConfig { applicationId \"com.gradle.app\" } }\n",
    ));
    let account = f.base.join("sa.json");
    ok(fs::write(&account, "{}"));
    let o = f.repo(
        &["prod", "android", f.p(), "--yes"],
        "",
        &[(
            "PLAY_SERVICE_ACCOUNT_PATH",
            account.to_str().unwrap_or_default(),
        )],
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let calls = f.calls();
    assert_eq!(calls[0], "flutter build appbundle --release");
    assert!(
        calls[1].starts_with("py /s/play_upload.py --aab ")
            && calls[1].ends_with("--track production"),
        "{calls:?}"
    );
    assert_eq!(calls[2], "  env: pkg=com.gradle.app creds= stores=");
}

#[test]
fn snapcraft_receives_its_credentials_from_the_environment_or_the_secret_store() {
    let f = fx();
    touch(&f.project, "build/snap/a.snap");
    let o = f.repo(
        &["test", "linux", f.p(), "--yes"],
        "",
        &[("SNAPCRAFT_STORE_CREDENTIALS", "tok")],
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(
        f.calls()
            .iter()
            .any(|c| c == "  env: pkg= creds=tok stores="),
        "{:?}",
        f.calls()
    );
    ok(fs::remove_file(f.base.join("calls.log")));
    let o = f.repo(&["prod", "linux", f.p(), "--yes"], "", &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert!(
        f.calls()
            .iter()
            .any(|c| c == "  env: pkg= creds=from-secret stores="),
        "{:?}",
        f.calls()
    );
}

#[test]
fn a_failing_tool_fails_the_command_and_names_the_target_and_the_program() {
    let f = fx();
    let o = f.repo(
        &["prod", "ios", f.p(), "--yes"],
        "",
        &[("FLUTTER_EXIT", "3")],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("deploy ios: flutter build ipa --release failed"),
        "{}",
        err(&o)
    );
    assert!(
        err(&o).contains("flutter exited with status 3"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_missing_wrangler_says_how_to_install_it() {
    let f = fx();
    touch(&f.project, "dist/index.html");
    ok(fs::remove_file(f.base.join("bin/wrangler")));
    let o = f.repo(&["prod", "web", f.p(), "--yes"], "", &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("Install it with: dotfiles add --manager npm wrangler"),
        "{}",
        err(&o)
    );
}

#[test]
fn all_plans_the_seven_targets_on_a_dry_run() {
    let f = fx();
    touch(&f.project, "dist/index.html");
    let o = f.repo(&["test", "all", f.p(), "--dry-run"], "", &[]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(out(&o).lines().count(), 7, "{}", out(&o));
    assert!(f.calls().is_empty());
}

#[test]
fn all_reports_every_failing_target_and_exits_1() {
    let f = fx();
    let o = f.repo(
        &["test", "all", f.p(), "--yes"],
        "",
        &[("FLUTTER_EXIT", "1")],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains(
            "deploy: 7 of 7 target(s) failed (web, ios, android, mas, homebrew, windows, linux)"
        ),
        "{}",
        err(&o)
    );
}

#[test]
fn store_page_dry_run_prints_the_push_for_ios() {
    let f = fx();
    ok(fs::create_dir_all(f.project.join("stores")));
    let o = f.repo(
        &["prod", "ios", f.p(), "--store-page", "--dry-run"],
        "",
        &[],
    );
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        out(&o),
        format!(
            "STORES_DIR={}/stores asc_upload.py (App Store page of com.x, no submission)\n",
            f.project.display()
        )
    );
}

#[test]
fn a_path_that_is_not_a_directory_is_refused() {
    let f = fx();
    let o = f.repo(&["prod", "web", "/nonexistent/p", "--yes"], "", &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("not a directory: /nonexistent/p"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_wrangler_that_is_not_an_executable_file_does_not_count() {
    let f = fx();
    touch(&f.project, "dist/index.html");
    ok(fs::remove_file(f.base.join("bin/wrangler")));
    ok(fs::write(f.base.join("bin/wrangler"), "#!/bin/sh\n"));
    ok(fs::set_permissions(
        f.base.join("bin/wrangler"),
        fs::Permissions::from_mode(0o644),
    ));
    let o = f.repo(&["prod", "web", f.p(), "--yes"], "", &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("wrangler not found"), "{}", err(&o));
    ok(fs::remove_file(f.base.join("bin/wrangler")));
    ok(fs::create_dir(f.base.join("bin/wrangler")));
    let o = f.repo(&["prod", "web", f.p(), "--yes"], "", &[]);
    assert!(
        err(&o).contains("wrangler not found"),
        "a directory is not a program: {}",
        err(&o)
    );
    assert!(f.calls().is_empty());
}
