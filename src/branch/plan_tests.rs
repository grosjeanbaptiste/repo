use super::*;

fn opts(from: &str, to: &str, delete_old: bool) -> Opts {
    Opts { from: from.into(), to: to.into(), delete_old }
}

fn lines(steps: &[Step]) -> Vec<String> {
    steps.iter().map(Step::line).collect()
}

#[test]
fn a_local_only_plan_renames_the_branch() {
    let steps = plan(&opts("master", "main", false), None);
    assert_eq!(lines(&steps), ["git branch -m master main"]);
}

#[test]
fn a_remote_plan_pushes_switches_the_default_and_refreshes_origin_head() {
    let steps = plan(&opts("master", "main", false), Some("me/ss"));
    assert_eq!(
        lines(&steps),
        [
            "git branch -m master main",
            "git push -u origin main",
            "gh repo edit me/ss --default-branch main",
            "git remote set-head origin -a",
        ]
    );
}

#[test]
fn deleting_the_old_remote_branch_comes_after_the_default_switch() {
    let steps = plan(&opts("master", "main", true), Some("me/ss"));
    assert_eq!(
        lines(&steps),
        [
            "git branch -m master main",
            "git push -u origin main",
            "gh repo edit me/ss --default-branch main",
            "git push origin --delete master",
            "git remote set-head origin -a",
        ]
    );
}

#[test]
fn the_old_remote_branch_is_kept_unless_asked() {
    let steps = plan(&opts("master", "main", false), Some("me/ss"));
    assert!(lines(&steps).iter().all(|l| !l.contains("--delete")));
}

#[test]
fn good_branch_names_are_accepted() {
    for name in ["main", "master", "trunk", "release/1.0", "feat_x-2", "v1.2"] {
        assert!(valid_branch(name).is_ok(), "{name}");
    }
}

#[test]
fn bad_branch_names_are_refused_with_the_name_in_the_message() {
    for name in [
        "", "-x", "--force", "a b", "a..b", "a~1", "a^", "a:b", "a?", "a*", "a[", "a\\b", "/a",
        "a/", "a.lock", "a\tb", "a\nb", "@", "a@{b", ".hidden", "a//b",
    ] {
        let err = valid_branch(name).err().unwrap_or_else(|| panic!("expected an error: {}", name)).to_string();
        assert!(err.contains(&format!("{name:?}")), "{name:?} -> {err}");
    }
}

#[test]
fn renaming_a_branch_to_itself_is_refused() {
    let err = validate(&opts("main", "main", false)).err().unwrap_or_else(|| panic!("expected an error: {}", "same")).to_string();
    assert!(err.contains("main"), "{err}");
}
