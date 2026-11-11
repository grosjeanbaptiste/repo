use super::*;
use crate::deploy::fake::{Fake, message};
use crate::deploy::target::{Env, Target};
use crate::deploy::vars::Vars;
use std::fs;
use std::path::Path;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

struct Out {
    result: Result<()>,
    tool: Fake,
    out: String,
}

fn go(root: &Path, choice: Choice, dry_run: bool, extra: &[(&str, &str)], tool: Fake) -> Out {
    let vars = Vars::of(&[&[("STORES_PYTHON", "/py"), ("STORES_SCRIPTS_DIR", "/s")][..], extra].concat(), &root.join("home"));
    let (mut out, mut err) = (vec![], vec![]);
    let result = {
        let mut c = Ctx { env: Env::Prod, root, vars: &vars, tool: &tool, out: &mut out, err: &mut err };
        push(&mut c, choice, dry_run)
    };
    Out { result, tool, out: String::from_utf8_lossy(&out).into_owned() }
}

fn project(stores: bool) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    if stores {
        ok(fs::create_dir_all(d.path().join("stores")));
    }
    d
}

const IOS: Choice = Choice::One(Target::Ios);
const ANDROID: Choice = Choice::One(Target::Android);

fn sa(d: &Path) -> String {
    let p = d.join("sa.json");
    ok(fs::write(&p, "{}"));
    p.display().to_string()
}

#[test]
fn only_ios_and_android_have_a_store_page() {
    let d = project(true);
    for (choice, name) in [(Choice::One(Target::Web), "web"), (Choice::One(Target::Mas), "mas"), (Choice::All, "all")] {
        let o = go(d.path(), choice, false, &[], Fake::default());
        assert_eq!(message(&o.result), format!("--store-page applies to ios and android, not '{name}'."));
    }
}

#[test]
fn the_stores_folder_must_exist() {
    let d = project(false);
    let o = go(d.path(), IOS, false, &[("ASC_BUNDLE_ID", "b")], Fake::default());
    assert_eq!(message(&o.result), format!("--store-page reads {}/stores, which does not exist.", d.path().display()));
}

#[test]
fn ios_pushes_the_app_store_page_through_asc_upload_without_submitting() {
    let d = project(true);
    let o = go(d.path(), IOS, false, &[("ASC_BUNDLE_ID", "com.x")], Fake::default());
    assert!(o.result.is_ok(), "{}", message(&o.result));
    let runs = o.tool.runs();
    assert_eq!((runs.len(), runs[0].prog.as_str(), runs[0].args.clone()), (1, "/py", vec!["/s/asc_upload.py".to_string()]));
    assert_eq!(runs[0].env, [("STORES_DIR".to_string(), format!("{}/stores", d.path().display()))]);
}

#[test]
fn ios_needs_the_bundle_id() {
    let d = project(true);
    let o = go(d.path(), IOS, false, &[], Fake::default());
    assert_eq!(message(&o.result), "--store-page ios needs ASC_BUNDLE_ID (per-project, e.g. .envrc).");
    assert!(o.tool.runs().is_empty());
}

#[test]
fn android_pushes_the_play_listing_with_package_and_account() {
    let d = project(true);
    let account = sa(d.path());
    let extra = [("PLAY_PACKAGE_NAME", "com.x"), ("PLAY_SERVICE_ACCOUNT_PATH", account.as_str())];
    let o = go(d.path(), ANDROID, false, &extra, Fake::default());
    assert!(o.result.is_ok(), "{}", message(&o.result));
    let runs = o.tool.runs();
    assert_eq!(runs[0].args, ["/s/play_upload.py"]);
    assert_eq!(
        runs[0].env,
        [
            ("STORES_DIR".to_string(), format!("{}/stores", d.path().display())),
            ("PLAY_PACKAGE_NAME".to_string(), "com.x".to_string()),
            ("PLAY_SERVICE_ACCOUNT_PATH".to_string(), account),
        ]
    );
}

#[test]
fn android_needs_a_package_then_a_service_account() {
    let d = project(true);
    let o = go(d.path(), ANDROID, false, &[], Fake::default());
    assert_eq!(message(&o.result), "cannot resolve the Android package (set PLAY_PACKAGE_NAME).");
    let o = go(d.path(), ANDROID, false, &[("PLAY_PACKAGE_NAME", "com.x"), ("PLAY_SERVICE_ACCOUNT_PATH", "/nope.json")], Fake::default());
    assert_eq!(message(&o.result), "no Play service account at /nope.json (dotfiles secret apply).");
}

#[test]
fn dry_run_prints_the_push_and_runs_nothing() {
    let d = project(true);
    let stores = format!("{}/stores", d.path().display());
    let o = go(d.path(), IOS, true, &[("ASC_BUNDLE_ID", "com.x")], Fake::default());
    assert_eq!(o.out, format!("STORES_DIR={stores} asc_upload.py (App Store page of com.x, no submission)\n"));
    let account = sa(d.path());
    let extra = [("PLAY_PACKAGE_NAME", "com.y"), ("PLAY_SERVICE_ACCOUNT_PATH", account.as_str())];
    let a = go(d.path(), ANDROID, true, &extra, Fake::default());
    assert_eq!(a.out, format!("STORES_DIR={stores} play_upload.py (Play listing of com.y, no bundle)\n"));
    assert!(o.tool.runs().is_empty() && a.tool.runs().is_empty());
}

#[test]
fn a_failing_uploader_is_an_error() {
    let d = project(true);
    let o = go(d.path(), IOS, false, &[("ASC_BUNDLE_ID", "b")], Fake { failing: vec!["/py".into()], ..Fake::default() });
    assert!(o.result.is_err());
}
