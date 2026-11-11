use super::*;
use crate::deploy::fake::Fake;
use crate::deploy::target::Env;
use crate::deploy::vars::Vars;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

struct Run {
    result: Result<()>,
    tool: Fake,
    home: tempfile::TempDir,
}

fn go(env: Env, kind: Kind, pairs: &[(&str, &str)], tool: Fake) -> Run {
    let home = ok(tempfile::tempdir());
    let vars = Vars::of(&[pairs, &[("STORES_PYTHON", "/py"), ("STORES_SCRIPTS_DIR", "/s")]].concat(), home.path());
    let (mut out, mut err) = (vec![], vec![]);
    let mut c = Ctx { env, root: Path::new("/proj"), vars: &vars, tool: &tool, out: &mut out, err: &mut err };
    let result = upload(&mut c, kind, Path::new("/proj/build/App.ipa"));
    Run { result, tool, home }
}

const KEYS: [(&str, &str); 2] = [("ASC_KEY_ID", "K1"), ("ASC_ISSUER_ID", "I1")];
const ALTOOL: &str = "xcrun altool --upload-app --type ios --file /proj/build/App.ipa --apiKey K1 --apiIssuer I1";

fn msg(r: &Run) -> String {
    r.result.as_ref().err().map(|e| e.to_string()).unwrap_or_default()
}

#[test]
fn test_uploads_to_testflight_and_never_submits() {
    let r = go(Env::Test, Kind::Ios, &KEYS, Fake::default());
    ok(r.result.as_ref().map_err(|e| e.to_string()));
    assert_eq!(r.tool.lines(), vec![ALTOOL.to_string()]);
}

#[test]
fn the_macos_kind_uploads_with_type_macos() {
    let r = go(Env::Test, Kind::Macos, &KEYS, Fake::default());
    assert_eq!(r.tool.lines(), vec![ALTOOL.replace("--type ios", "--type macos")]);
}

#[test]
fn prod_uploads_then_submits_the_ios_platform_through_the_stores_script() {
    let r = go(Env::Prod, Kind::Ios, &[KEYS[0], KEYS[1], ("ASC_BUNDLE_ID", "com.x")], Fake::default());
    assert!(r.result.is_ok());
    assert_eq!(r.tool.lines(), vec![ALTOOL.to_string(), "/py /s/asc_upload.py --submit --platform IOS".to_string()]);
}

#[test]
fn prod_macos_submits_the_mac_os_platform() {
    let r = go(Env::Prod, Kind::Macos, &[KEYS[0], KEYS[1], ("ASC_BUNDLE_ID", "com.x")], Fake::default());
    assert!(r.tool.lines()[1].ends_with("--platform MAC_OS"), "{:?}", r.tool.lines());
}

#[test]
fn prod_without_a_bundle_id_is_refused_before_anything_is_uploaded() {
    let r = go(Env::Prod, Kind::Ios, &KEYS, Fake::default());
    assert_eq!(msg(&r), "ASC_BUNDLE_ID must be set for a prod App Store release (per-project, e.g. .envrc). test targets TestFlight only.");
    assert!(r.tool.runs().is_empty());
}

#[test]
fn missing_credentials_are_named_and_nothing_runs() {
    for pairs in [vec![], vec![KEYS[0]], vec![KEYS[1]], vec![("ASC_KEY_ID", ""), KEYS[1]]] {
        let r = go(Env::Test, Kind::Ios, &pairs, Fake::default());
        assert_eq!(msg(&r), "ASC_KEY_ID/ASC_ISSUER_ID not set — source ~/.config/dotfiles/profile.local.sh.");
        assert!(r.tool.runs().is_empty());
    }
}

#[test]
fn a_failed_upload_is_an_error_and_prod_does_not_submit() {
    let t = Fake { failing: vec!["xcrun".into()], ..Fake::default() };
    let r = go(Env::Prod, Kind::Ios, &[KEYS[0], KEYS[1], ("ASC_BUNDLE_ID", "b")], t);
    assert!(r.result.is_err());
    assert_eq!(r.tool.runs().len(), 1);
}

#[test]
fn a_failed_submission_is_an_error() {
    let t = Fake { failing: vec!["/py".into()], ..Fake::default() };
    let r = go(Env::Prod, Kind::Ios, &[KEYS[0], KEYS[1], ("ASC_BUNDLE_ID", "b")], t);
    assert!(r.result.is_err());
}

#[test]
fn the_private_key_is_staged_where_altool_looks_for_it() {
    let home = ok(tempfile::tempdir());
    let key = home.path().join("custom.p8");
    ok(fs::write(&key, "SECRET"));
    let stage = home.path().join("stage");
    let pairs = [KEYS[0], KEYS[1], ("ASC_PRIVATE_KEY_PATH", key.to_str().unwrap_or_default()), ("ASC_KEY_STAGE_DIR", stage.to_str().unwrap_or_default())];
    let r = go(Env::Test, Kind::Ios, &pairs, Fake::default());
    assert!(r.result.is_ok());
    assert_eq!(ok(fs::read_to_string(stage.join("AuthKey_K1.p8"))), "SECRET");
}

#[test]
fn the_default_key_and_stage_live_under_home_and_an_absent_key_is_not_staged() {
    let home = ok(tempfile::tempdir());
    ok(fs::create_dir_all(home.path().join(".config/asc")));
    ok(fs::write(home.path().join(".config/asc/AuthKey.p8"), "K"));
    let tool = Fake::default();
    let vars = Vars::of(&[KEYS[0], KEYS[1]], home.path());
    let (mut out, mut err) = (vec![], vec![]);
    let mut c = Ctx { env: Env::Test, root: Path::new("/p"), vars: &vars, tool: &tool, out: &mut out, err: &mut err };
    ok(upload(&mut c, Kind::Ios, Path::new("/a.ipa")));
    assert_eq!(ok(fs::read_to_string(home.path().join(".appstoreconnect/private_keys/AuthKey_K1.p8"))), "K");
    let r = go(Env::Test, Kind::Ios, &KEYS, Fake::default());
    assert!(r.result.is_ok() && !r.home.path().join(".appstoreconnect").exists());
}
