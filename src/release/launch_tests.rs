use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn the_command_line_is_bash_snippet_profile_target_script_then_forwarded_arguments() {
    let got = argv(Path::new("/m/shell/profile.sh"), Path::new("/p"), &v(&["python3", "/p/r.py"]), &v(&["--skip", "x y"]));
    assert_eq!(&got[..4], &v(&["bash", "-c", SNIPPET, "release"])[..]);
    assert_eq!(&got[4..], &v(&["/m/shell/profile.sh", "/p", "python3", "/p/r.py", "--skip", "x y"])[..]);
}

#[test]
fn the_snippet_exports_what_it_sources_restores_the_directory_and_execs_the_rest() {
    for part in ["set -a", "set +a", "pushd \"$2\"", "popd", "shift 2", "exec \"$@\""] {
        assert!(SNIPPET.contains(part), "missing {part}");
    }
}
