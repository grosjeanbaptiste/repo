//! The text `repo push` prints: header and table, the push candidates, the summary.
use super::rows::{Row, State};
use std::path::Path;

pub fn table(target: &Path, rows: &[Row]) -> String {
    let width = rows
        .iter()
        .map(|r| r.rel.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let mut out = format!(
        "==> repo push\n    scan root : {}\n    repos     : {} found\n\n",
        target.display(),
        rows.len()
    );
    out.push_str(&format!(
        "    {:<width$}  {:<12}  {:<14}  {}\n",
        "PATH", "STATE", "BRANCH", "REMOTE"
    ));
    out.push_str(&format!(
        "    {:<width$}  {:<12}  {:<14}  {}\n",
        "-".repeat(width),
        "-".repeat(12),
        "-".repeat(14),
        "------"
    ));
    for r in rows {
        out.push_str(&format!(
            "    {:<width$}  {:<12}  {:<14}  {}\n",
            r.rel,
            r.state.label(),
            r.branch,
            r.remote
        ));
    }
    out
}

/// Rows that can be pushed: a clean tree with commits ahead of its upstream.
pub fn candidates(rows: &[Row]) -> Vec<(&Row, u64)> {
    rows.iter()
        .filter_map(|r| match r.state {
            State::Ahead(n) => Some((r, n)),
            _ => None,
        })
        .collect()
}

/// "N repo(s) to push:" and one line per candidate.
pub fn candidate_list(candidates: &[(&Row, u64)]) -> String {
    let mut out = format!("==> {} repo(s) to push:\n", candidates.len());
    for (r, n) in candidates {
        out.push_str(&format!(
            "        {} ({}, {n} commits ahead)\n",
            r.rel, r.branch
        ));
    }
    out
}

pub fn summary(pushed: usize, total: usize, failed: usize) -> String {
    format!("==> pushed {pushed}/{total} ({failed} failed)")
}

#[cfg(test)]
mod tests {
    include!("report_tests.rs");
}
