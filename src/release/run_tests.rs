use super::*;
use std::cell::RefCell;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

#[derive(Default)]
struct Spy {
    seen: RefCell<Vec<Vec<String>>>,
    fail: bool,
}

impl Launch for Spy {
    fn launch(&self, argv: &[String]) -> Result<()> {
        self.seen.borrow_mut().push(argv.to_vec());
        if self.fail { bail!("exec failed") } else { Ok(()) }
    }
}

fn project() -> (tempfile::TempDir, PathBuf) {
    let d = ok(tempfile::tempdir());
    let root = ok(d.path().canonicalize());
    ok(fs::create_dir_all(root.join("stores/scripts")));
    ok(fs::write(root.join("stores/scripts/tf_release.py"), "x"));
    (d, root)
}

fn opts(path: &Path) -> Opts {
    Opts { path: Some(path.to_string_lossy().into_owned()), ..Opts::default() }
}

fn go(o: &Opts, spy: &Spy, audit: Result<(), &'static str>) -> (Result<()>, String, String) {
    let audit_fn = move |_: &Path, out: &mut dyn Write, err: &mut dyn Write| -> Result<()> {
        ok(writeln!(out, "AUDIT REPORT"));
        ok(writeln!(err, "audit warning"));
        audit.map_err(|m| anyhow!(m))
    };
    let ports = Ports { profile: PathBuf::from("/m/shell/profile.sh"), audit: &audit_fn, launch: spy };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = run(o, &ports, &mut out, &mut err);
    (r, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn the_conventional_script_is_launched_with_the_profile_target_and_forwarded_arguments() {
    let (_d, root) = project();
    let spy = Spy::default();
    let o = Opts { forward: vec!["--skip-upload".into()], ..opts(&root) };
    let (r, out, err) = go(&o, &spy, Ok(()));
    assert!(r.is_ok(), "{r:?}");
    assert!(out.is_empty() && err.is_empty(), "{out}{err}");
    let seen = spy.seen.borrow();
    assert_eq!(seen.len(), 1);
    let want = [
        "/m/shell/profile.sh".to_string(),
        root.to_string_lossy().into_owned(),
        "python3".into(),
        root.join("stores/scripts/tf_release.py").to_string_lossy().into_owned(),
        "--skip-upload".into(),
    ];
    assert_eq!(&seen[0][4..], &want[..]);
}

#[test]
fn a_missing_directory_fails_before_anything_else() {
    let (_d, root) = project();
    let spy = Spy::default();
    let (r, ..) = go(&opts(&root.join("gone")), &spy, Ok(()));
    assert!(r.err().is_some_and(|e| e.to_string().starts_with("release: path is not a directory: ")));
    assert!(spy.seen.borrow().is_empty());
}

#[test]
fn no_script_fails_without_launching() {
    let d = ok(tempfile::tempdir());
    let spy = Spy::default();
    let (r, ..) = go(&opts(d.path()), &spy, Ok(()));
    let msg = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(msg.starts_with("release: no release script found in "), "{msg}");
    assert!(msg.ends_with("Override with --script PATH if yours lives elsewhere."), "{msg}");
    assert!(spy.seen.borrow().is_empty());
}

#[test]
fn an_unrunnable_script_fails_without_launching() {
    let (_d, root) = project();
    ok(fs::write(root.join("ship"), "x"));
    let spy = Spy::default();
    let o = Opts { script: Some("ship".into()), ..opts(&root) };
    let (r, ..) = go(&o, &spy, Ok(()));
    let msg = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(msg.starts_with("release: cannot determine how to run "), "{msg}");
    assert!(spy.seen.borrow().is_empty());
}

#[test]
fn a_passing_audit_prints_its_report_and_a_blank_line_then_launches() {
    let (_d, root) = project();
    let spy = Spy::default();
    let o = Opts { require_audit: true, ..opts(&root) };
    let (r, out, err) = go(&o, &spy, Ok(()));
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(out, "AUDIT REPORT\n\n");
    assert_eq!(err, "audit warning\n");
    assert_eq!(spy.seen.borrow().len(), 1);
}

#[test]
fn a_failing_audit_aborts_with_its_reason_and_never_launches() {
    let (_d, root) = project();
    let spy = Spy::default();
    let o = Opts { require_audit: true, ..opts(&root) };
    let (r, out, _) = go(&o, &spy, Err("audit: 1 variable(s) missing"));
    assert_eq!(out, "AUDIT REPORT\n");
    let msg = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert_eq!(msg, format!("release: audit failed for {} — aborting: audit: 1 variable(s) missing", root.display()));
    assert!(spy.seen.borrow().is_empty());
}

#[test]
fn without_require_audit_the_audit_is_not_run() {
    let (_d, root) = project();
    let spy = Spy::default();
    let (_, out, err) = go(&opts(&root), &spy, Err("never asked"));
    assert!(out.is_empty() && err.is_empty());
}

#[test]
fn a_launch_that_fails_is_returned() {
    let (_d, root) = project();
    let spy = Spy { fail: true, ..Spy::default() };
    let (r, ..) = go(&opts(&root), &spy, Ok(()));
    assert_eq!(r.err().map(|e| e.to_string()).as_deref(), Some("exec failed"));
}

#[test]
fn the_script_option_is_resolved_against_the_target() {
    let (_d, root) = project();
    ok(fs::write(root.join("ship.sh"), "x"));
    let spy = Spy::default();
    let o = Opts { script: Some("ship.sh".into()), ..opts(&root) };
    ok(go(&o, &spy, Ok(())).0);
    let seen = spy.seen.borrow();
    assert_eq!(&seen[0][6..], &["bash".to_string(), root.join("ship.sh").to_string_lossy().into_owned()][..]);
}
