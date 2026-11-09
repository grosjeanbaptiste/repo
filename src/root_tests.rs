use super::*;

fn home() -> Option<String> {
    Some("/home/me".to_string())
}

#[test]
fn tilde_alone_and_tilde_slash_expand_to_home() {
    assert_eq!(expand("~", home()).ok(), Some(PathBuf::from("/home/me")));
    assert_eq!(expand("~/Source", home()).ok(), Some(PathBuf::from("/home/me/Source")));
    assert_eq!(expand("~/a b/c", home()).ok(), Some(PathBuf::from("/home/me/a b/c")));
}

#[test]
fn other_paths_are_untouched() {
    for p in ["/abs/x", "rel/x", ".", "./x", "a~b", "x/~"] {
        assert_eq!(expand(p, home()).ok(), Some(PathBuf::from(p)), "{p}");
    }
}

#[test]
fn tilde_user_is_refused_rather_than_taken_literally() {
    let err = expand("~bob/x", home()).err().map(|e| e.to_string());
    assert!(err.is_some_and(|e| e.contains("~bob")));
}

#[test]
fn a_missing_home_is_named_in_the_error() {
    for p in ["~", "~/Source"] {
        let err = expand(p, None).err().map(|e| e.to_string());
        assert!(err.is_some_and(|e| e.contains("HOME")), "{p}");
    }
    assert!(expand("/abs", None).is_ok(), "no HOME needed without a tilde");
}
