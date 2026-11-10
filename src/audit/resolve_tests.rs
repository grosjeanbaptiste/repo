use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn well_known_variables_map_to_their_secret() {
    let table: [(&[&str], &str); 5] = [
        (&["GH_TOKEN", "GITHUB_TOKEN", "GITHUB_AUTH_TOKEN", "GH_API_TOKEN"], "gh-token"),
        (&["HF_TOKEN", "HUGGINGFACE_TOKEN", "HUGGING_FACE_TOKEN", "HUGGINGFACE_HUB_TOKEN"], "hf-token"),
        (&["GITLAB_TOKEN", "GL_TOKEN", "GITLAB_API_TOKEN", "GLAB_TOKEN", "CI_JOB_TOKEN"], "glab-config"),
        (&["NPM_TOKEN", "NODE_AUTH_TOKEN"], "npmrc"),
        (&["ASC_KEY_ID", "ASC_ISSUER_ID", "ASC_PRIVATE_KEY_PATH", "ASC_KEY_PATH"], "asc-api-key"),
    ];
    for (vars, secret) in table {
        for v in vars {
            assert_eq!(known_secret(v), Some(secret), "{v}");
        }
    }
}

#[test]
fn other_variables_and_other_cases_have_no_secret() {
    for v in ["DATABASE_URL", "gh_token", "GH_TOKEN ", "", "NPM_TOKENS"] {
        assert_eq!(known_secret(v), None, "{v:?}");
    }
}

fn st(var: &str, in_env: bool, enrolled: bool) -> Status {
    status(var, &|_| in_env, &|_| enrolled)
}

#[test]
fn the_environment_wins_over_the_store() {
    assert_eq!(st("GH_TOKEN", true, true), Status::InEnv);
    assert_eq!(st("DATABASE_URL", true, false), Status::InEnv);
}

#[test]
fn a_known_variable_with_an_enrolled_secret_is_in_dotfiles() {
    assert_eq!(st("GH_TOKEN", false, true), Status::InDotfiles("gh-token".into()));
}

#[test]
fn a_known_variable_without_its_secret_is_missing() {
    assert_eq!(st("GH_TOKEN", false, false), Status::Missing);
}

#[test]
fn an_unknown_variable_is_missing_whatever_the_store_holds() {
    assert_eq!(st("DATABASE_URL", false, true), Status::Missing);
}

#[test]
fn the_store_is_asked_for_the_secret_name_not_the_variable() {
    let asked = std::cell::RefCell::new(String::new());
    let r = status("NPM_TOKEN", &|_| false, &|s| {
        *asked.borrow_mut() = s.to_string();
        true
    });
    assert_eq!(r, Status::InDotfiles("npmrc".into()));
    assert_eq!(*asked.borrow(), "npmrc");
}

#[test]
fn the_environment_is_asked_for_the_variable_name() {
    let asked = std::cell::RefCell::new(String::new());
    status("FOO", &|v| {
        *asked.borrow_mut() = v.to_string();
        false
    }, &|_| false);
    assert_eq!(*asked.borrow(), "FOO");
}

#[test]
fn enrolled_means_a_readable_age_file_under_secrets() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("secrets")));
    ok(fs::write(d.path().join("secrets/npmrc.age"), "x"));
    assert!(enrolled(d.path(), "npmrc"));
    assert!(!enrolled(d.path(), "hf-token"));
    ok(fs::write(d.path().join("secrets/other"), "x"));
    assert!(!enrolled(d.path(), "other"), "needs the .age suffix");
}

#[test]
fn an_unreadable_secret_file_is_not_enrolled() {
    use std::os::unix::fs::PermissionsExt;
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("secrets")));
    let f = d.path().join("secrets/npmrc.age");
    ok(fs::write(&f, "x"));
    ok(fs::set_permissions(&f, fs::Permissions::from_mode(0o000)));
    let r = enrolled(d.path(), "npmrc");
    ok(fs::set_permissions(&f, fs::Permissions::from_mode(0o600)));
    assert!(!r);
}

#[test]
fn a_missing_state_root_means_nothing_is_enrolled() {
    assert!(!enrolled(Path::new("/no/such/root"), "npmrc"));
}
