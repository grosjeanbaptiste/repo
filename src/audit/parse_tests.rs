use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn env_names_are_the_left_side_of_assignments() {
    assert_eq!(env_names("A=1\nB_2=\n_C=x=y\n"), v(&["A", "B_2", "_C"]));
}

#[test]
fn env_names_allow_spaces_around_the_name_and_before_the_equals() {
    assert_eq!(env_names("  A = 1\n\tB\t=2\n"), v(&["A", "B"]));
}

#[test]
fn env_names_skip_comments_blank_lines_exports_and_bad_names() {
    let text = "# A=1\n\nexport B=2\n1C=3\n-D=4\nE F=5\nG\n=6\n";
    assert!(env_names(text).is_empty(), "{:?}", env_names(text));
}

#[test]
fn env_names_keep_duplicates_and_handle_crlf() {
    assert_eq!(env_names("A=1\r\nA=2\r\n"), v(&["A", "A"]));
}

#[test]
fn env_names_of_nothing_is_nothing() {
    assert!(env_names("").is_empty());
}

#[test]
fn workflow_names_take_secrets_vars_and_env() {
    let t = "a: ${{ secrets.TOKEN }}\nb: ${{vars.REGION}}\nc: ${{   env.HOME_DIR  }}\n";
    assert_eq!(workflow_names(t), v(&["TOKEN", "REGION", "HOME_DIR"]));
}

#[test]
fn workflow_names_find_several_on_one_line() {
    assert_eq!(workflow_names("x: ${{ secrets.A }}-${{ secrets.B }}"), v(&["A", "B"]));
}

#[test]
fn workflow_names_ignore_other_contexts_and_bad_names() {
    let t = "${{ github.token }}\n${{ matrix.os }}\n${{ secrets.1X }}\n${{ secrets. }}\n${{ secretsX.A }}\n";
    assert!(workflow_names(t).is_empty(), "{:?}", workflow_names(t));
}

#[test]
fn workflow_names_stop_at_the_end_of_the_identifier() {
    assert_eq!(workflow_names("${{ secrets.A_b2-c }}"), v(&["A_b2"]));
}

#[test]
fn workflow_names_need_the_expression_on_one_line() {
    assert!(workflow_names("${{\n secrets.A }}").is_empty());
}

#[test]
fn compose_names_take_braced_variables_with_or_without_modifiers() {
    let t = "a: ${DB_URL}\nb: ${PORT:-80}\nc: ${KEY:?missing}\nd: ${plain}";
    assert_eq!(compose_names(t), v(&["DB_URL", "PORT", "KEY", "plain"]));
}

#[test]
fn compose_names_find_several_on_one_line_and_keep_duplicates() {
    assert_eq!(compose_names("${A}${B} ${A}"), v(&["A", "B", "A"]));
}

#[test]
fn compose_names_ignore_bare_dollars_and_bad_starts() {
    assert!(compose_names("$A $1 ${1A} ${-x} ${} $$").is_empty());
}

#[test]
fn compose_names_ignore_github_expressions() {
    assert!(compose_names("${{ secrets.A }}").is_empty());
}
