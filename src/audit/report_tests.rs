use super::super::diag::{fail, ok as okf, warn};
use super::*;

fn r(var: &str, source: &str) -> Ref {
    Ref { var: var.into(), source: source.into() }
}

fn draw(refs: &[Ref], findings: &[Finding], status: &dyn Fn(&str) -> Status) -> (Tally, String) {
    let mut out = Vec::new();
    let t = render(Path::new("/p"), refs, findings, status, &mut out).unwrap_or_else(|e| panic!("{e}"));
    (t, String::from_utf8_lossy(&out).into_owned())
}

fn by_name(var: &str) -> Status {
    match var {
        "A" => Status::InEnv,
        "GH_TOKEN" => Status::InDotfiles("gh-token".into()),
        _ => Status::Missing,
    }
}

#[test]
fn an_empty_audit_says_so_and_prints_no_table() {
    let (t, out) = draw(&[], &[], &by_name);
    assert_eq!(out, "== auditing /p ==\n  (no env vars / secrets referenced in this tree)\n");
    assert_eq!(t, Tally::default());
}

#[test]
fn variables_are_listed_with_status_sources_and_counts() {
    let refs = [r("A", ".env.example"), r("A", "ci.yml"), r("GH_TOKEN", "ci.yml"), r("ZED", "Dockerfile")];
    let (t, out) = draw(&refs, &[], &by_name);
    let want = format!(
        "== auditing /p ==\n\n  {:<32} {:<14} SOURCES / HINT\n  {:<32} {:<14} --------------\n  {:<32} {:<14} .env.example, ci.yml\n  {:<32} {:<14} ci.yml  (enrolled as gh-token)\n  {:<32} {:<14} Dockerfile\n\n  1 in env, 1 in dotfiles (not exported), 1 missing\n",
        "VAR", "STATUS", "---", "------", "A", "in env", "GH_TOKEN", "in dotfiles", "ZED", "missing"
    );
    assert_eq!(out, want);
    assert_eq!(t, Tally { missing: 1, failed: 0 });
}

#[test]
fn findings_get_their_own_table_and_counts() {
    let f = [okf("fine"), warn("hmm"), fail("bad"), warn("again")];
    let (t, out) = draw(&[], &f, &by_name);
    let want = format!(
        "== auditing /p ==\n  (no env vars / secrets referenced in this tree)\n\n  {:<6} FINDING\n  {:<6} -------\n  {:<6} fine\n  {:<6} hmm\n  {:<6} bad\n  {:<6} again\n\n  1 ok, 2 warn, 1 fail\n",
        "STATUS", "------", "OK", "WARN", "FAIL", "WARN"
    );
    assert_eq!(out, want);
    assert_eq!(t, Tally { missing: 0, failed: 1 });
}

#[test]
fn no_findings_prints_no_findings_table() {
    let (_, out) = draw(&[r("A", "x")], &[], &by_name);
    assert!(!out.contains("FINDING"), "{out}");
}

#[test]
fn the_tally_is_failing_only_for_missing_or_fail() {
    assert_eq!(draw(&[r("Q", "x")], &[warn("w")], &by_name).0, Tally { missing: 1, failed: 0 });
    assert_eq!(draw(&[r("A", "x")], &[warn("w"), okf("o")], &by_name).0, Tally::default());
}

#[test]
fn a_long_variable_name_still_gets_one_space_before_the_status() {
    let long = "A".repeat(40);
    let (_, out) = draw(&[r(&long, "x")], &[], &|_| Status::Missing);
    assert!(out.contains(&format!("  {long} missing        x\n")), "{out}");
}

#[test]
fn level_is_displayed_padded() {
    assert_eq!(format!("{:<6}|", Level::Ok), "OK    |");
}
