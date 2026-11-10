use super::*;
use std::path::PathBuf;

fn row(rel: &str, branch: &str, remote: &str, state: State) -> Row {
    Row { repo: PathBuf::from("/r").join(rel), rel: rel.into(), branch: branch.into(), remote: remote.into(), state }
}

#[test]
fn the_table_has_a_header_a_rule_and_one_line_per_repository() {
    let rows = [row(".", "main", "git@x:a.git", State::UpToDate), row("sub/inner", "dev", "(no origin)", State::Ahead(2))];
    let want = "==> repo push\n    scan root : /r\n    repos     : 2 found\n\n\
        \x20   PATH       STATE         BRANCH          REMOTE\n\
        \x20   ---------  ------------  --------------  ------\n\
        \x20   .          up-to-date    main            git@x:a.git\n\
        \x20   sub/inner  ahead:2       dev             (no origin)\n";
    assert_eq!(table(Path::new("/r"), &rows), want);
}

#[test]
fn the_path_column_is_at_least_four_wide() {
    let t = table(Path::new("/r"), &[row(".", "main", "o", State::Dirty)]);
    assert!(t.contains("    PATH  STATE"), "{t}");
    assert!(t.contains("    ----  ------------"), "{t}");
}

#[test]
fn candidates_are_clean_trees_ahead_of_their_upstream_only() {
    let rows = [
        row("a", "m", "o", State::Ahead(3)),
        row("b", "m", "o", State::Dirty),
        row("c", "m", "o", State::NoUpstream),
        row("d", "m", "o", State::UpToDate),
        row("e", "m", "o", State::Error("boom".into())),
        row("f", "m", "o", State::Ahead(1)),
    ];
    let c: Vec<(&str, u64)> = candidates(&rows).into_iter().map(|(r, n)| (r.rel.as_str(), n)).collect();
    assert_eq!(c, [("a", 3), ("f", 1)]);
}

#[test]
fn candidate_list_names_repo_branch_and_count() {
    let rows = [row("a", "main", "o", State::Ahead(3))];
    let c = candidates(&rows);
    assert_eq!(candidate_list(&c), "==> 1 repo(s) to push:\n        a (main, 3 commits ahead)\n");
}

#[test]
fn summary_counts_pushed_total_and_failed() {
    assert_eq!(summary(2, 3, 1), "==> pushed 2/3 (1 failed)");
    assert_eq!(summary(0, 0, 0), "==> pushed 0/0 (0 failed)");
}
