# Context — 2026-11-09

- Rust CLI. Reuses: `actname` lib (git dep pinned to Actname `001768a`, includes the git/dev-tooling lexicon, PR #1 merged), `dotfiles project` (setup/list/status/pr/doctor), `gh`. Source: `Cargo.toml`, `src/`.
- Replaced an earlier bash+bats version the same day (same behaviour, now delegating scaffold to `dotfiles project setup`).
- Cargo uses the git CLI to fetch Actname over ssh (`.cargo/config.toml`).
- Not verified: a real `repo new` without `--dry-run` (creates a GitHub repo).
- Directory still named `repo` (placeholder); not pushed to GitHub.

- 2026-11-09: name availability now comes from the `actit` lib (pinned to Actit `b2044d4`): a name is skipped if its directory exists or if any GitHub owner has a repo of that name. The own `src/github.rs` is gone.
