use super::*;

const META: &str = r#"{"packages":[
  {"name":"ss","manifest_path":"/w/ss/Cargo.toml"},
  {"name":"ss-ui","manifest_path":"/w/ss/ui/Cargo.toml"}]}"#;

fn why(r: Result<PathBuf>) -> String {
    r.err().map(|e| format!("{e:#}")).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn a_member_is_found_by_name_and_gives_its_directory() {
    let got = member_dir(META, "ss-ui", Path::new("/w/ss")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(got, Path::new("/w/ss/ui"));
    let root = member_dir(META, "ss", Path::new("/w/ss")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(root, Path::new("/w/ss"));
}

#[test]
fn a_name_that_is_only_a_prefix_is_not_a_match() {
    assert_eq!(why(member_dir(META, "ss-u", Path::new("/w/ss"))), "no package ss-u in /w/ss");
}

#[test]
fn an_unknown_package_names_it_and_the_project() {
    assert_eq!(why(member_dir(META, "nope", Path::new("/w/ss"))), "no package nope in /w/ss");
}

#[test]
fn metadata_that_is_not_json_is_an_error() {
    assert!(why(member_dir("oops", "ss", Path::new("/w/ss"))).contains("cargo metadata"));
}

#[test]
fn metadata_without_packages_is_an_error_not_an_empty_answer() {
    assert!(why(member_dir("{}", "ss", Path::new("/w/ss"))).contains("packages"));
}

#[test]
fn a_matching_package_without_a_manifest_path_is_an_error() {
    let meta = r#"{"packages":[{"name":"ss"}]}"#;
    assert!(why(member_dir(meta, "ss", Path::new("/w/ss"))).contains("manifest_path"));
}
