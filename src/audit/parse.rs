//! Variable names referenced by one file, in order, duplicates kept.

/// The `[A-Za-z_][A-Za-z0-9_]*` at the start of `s`, or "".
fn ident(s: &str) -> &str {
    let ok = |c: char, first: bool| {
        c == '_' || c.is_ascii_alphabetic() || (!first && c.is_ascii_digit())
    };
    let end = s
        .char_indices()
        .find(|&(i, c)| !ok(c, i == 0))
        .map_or(s.len(), |(i, _)| i);
    &s[..end]
}

fn blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// `NAME=…` lines of `.env.example` and its siblings (spaces around the name allowed).
pub fn env_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim_start_matches(blank);
            let name = ident(rest);
            let after = rest[name.len()..].trim_start_matches(blank);
            (!name.is_empty() && after.starts_with('=')).then(|| name.to_string())
        })
        .collect()
}

/// `${{ secrets.X }}`, `${{ vars.X }}`, `${{ env.X }}` of a GitHub workflow.
pub fn workflow_names(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in text.lines() {
        for (i, _) in line.match_indices("${{") {
            let rest = line[i + 3..].trim_start_matches(blank);
            let ctx = ["secrets.", "vars.", "env."]
                .into_iter()
                .find_map(|c| rest.strip_prefix(c));
            let name = ctx.map_or("", ident);
            if !name.is_empty() {
                found.push(name.to_string());
            }
        }
    }
    found
}

/// `${VAR}`, `${VAR:-default}`, `${VAR:?msg}` of a compose file or Dockerfile.
pub fn compose_names(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in text.lines() {
        for (i, _) in line.match_indices("${") {
            let name = ident(&line[i + 2..]);
            if !name.is_empty() {
                found.push(name.to_string());
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    include!("parse_tests.rs");
}
