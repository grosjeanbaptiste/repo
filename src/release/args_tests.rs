use super::*;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn run(a: &[&str]) -> Opts {
    match parse(&args(a)) {
        Ok(Parsed::Run(o)) => o,
        other => panic!("{other:?}"),
    }
}

fn why(a: &[&str]) -> String {
    parse(&args(a)).err().map(|e| e.to_string()).unwrap_or_else(|| panic!("expected an error"))
}

#[test]
fn no_argument_runs_with_defaults() {
    assert_eq!(run(&[]), Opts::default());
}

#[test]
fn our_flags_are_read_in_any_order() {
    let o = run(&["--require-audit", "--path", "/p", "--script", "s.py"]);
    assert_eq!(
        o,
        Opts { require_audit: true, path: Some("/p".into()), script: Some("s.py".into()), forward: vec![] }
    );
    let o = run(&["--script", "s.py", "--path", "/p"]);
    assert_eq!((o.path.as_deref(), o.script.as_deref(), o.require_audit), (Some("/p"), Some("s.py"), false));
}

#[test]
fn the_equals_forms_work() {
    let o = run(&["--path=/p", "--script=a=b.py"]);
    assert_eq!((o.path.as_deref(), o.script.as_deref()), (Some("/p"), Some("a=b.py")));
}

#[test]
fn the_last_value_wins() {
    assert_eq!(run(&["--path", "/a", "--path", "/b"]).path.as_deref(), Some("/b"));
}

#[test]
fn an_unknown_flag_and_everything_after_it_is_forwarded() {
    let o = run(&["--skip-upload", "--path", "/p", "x"]);
    assert_eq!(o.forward, args(&["--skip-upload", "--path", "/p", "x"]));
    assert_eq!(o.path, None);
}

#[test]
fn a_positional_stops_our_parsing() {
    let o = run(&["--require-audit", "beta", "--require-audit"]);
    assert!(o.require_audit);
    assert_eq!(o.forward, args(&["beta", "--require-audit"]));
}

#[test]
fn a_double_dash_stops_our_parsing_and_is_dropped() {
    let o = run(&["--path", "/p", "--", "--path", "other"]);
    assert_eq!(o.path.as_deref(), Some("/p"));
    assert_eq!(o.forward, args(&["--path", "other"]));
}

#[test]
fn a_flag_without_a_value_is_an_error_naming_it() {
    assert_eq!(why(&["--path"]), "--path needs a value");
    assert_eq!(why(&["--script"]), "--script needs a value");
}

#[test]
fn help_is_recognised_as_flag_or_word_but_only_before_the_script_arguments() {
    for h in ["-h", "--help", "help"] {
        assert_eq!(parse(&args(&[h])).ok(), Some(Parsed::Help));
    }
    assert_eq!(parse(&args(&["--require-audit", "--help"])).ok(), Some(Parsed::Help));
    assert_eq!(run(&["beta", "--help"]).forward, args(&["beta", "--help"]));
}
