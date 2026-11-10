//! `{{var}}` interpolation for `.tmpl` files and `{{project_name}}` in paths.
use std::collections::BTreeMap;

pub type Vars = BTreeMap<String, String>;

/// Replaces every `{{key}}` whose key is in `vars`, in one pass: a value that
/// itself contains `{{x}}` is never re-expanded. Unknown placeholders stay.
pub fn render(text: &str, vars: &Vars) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        match tail.find("}}").map(|end| (&tail[..end], end)) {
            Some((key, end)) if vars.contains_key(key) => {
                out.push_str(&vars[key]);
                rest = &tail[end + 2..];
            }
            _ => {
                out.push('{');
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vars {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn replaces_known_keys_everywhere() {
        let v = vars(&[("project_name", "ifdck"), ("year", "2026")]);
        assert_eq!(
            render("# {{project_name}} {{year}} {{project_name}}", &v),
            "# ifdck 2026 ifdck"
        );
    }

    #[test]
    fn unknown_placeholders_are_left_untouched() {
        assert_eq!(
            render("{{nope}} {{ user }}", &vars(&[("user", "a")])),
            "{{nope}} {{ user }}"
        );
    }

    #[test]
    fn values_are_not_re_expanded() {
        let v = vars(&[("user", "{{email}}"), ("email", "x@y")]);
        assert_eq!(render("{{user}}", &v), "{{email}}");
    }

    #[test]
    fn special_characters_in_values_are_literal() {
        let v = vars(&[("user", "a/b&c\\1 $x")]);
        assert_eq!(render("{{user}}", &v), "a/b&c\\1 $x");
    }

    #[test]
    fn unterminated_and_nested_braces_survive() {
        let v = vars(&[("a", "1")]);
        assert_eq!(render("{{a", &v), "{{a");
        assert_eq!(render("{{{a}}}", &v), "{1}");
        assert_eq!(render("}}{{a}}{{", &v), "}}1{{");
        assert_eq!(render("", &v), "");
    }

    #[test]
    fn multibyte_text_is_preserved() {
        assert_eq!(render("é{{a}}ü", &vars(&[("a", "→")])), "é→ü");
    }
}
