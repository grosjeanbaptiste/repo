# Context — 2026-11-09

- Rust CLI. Reuses: `actname` lib (git dep pinned to Actname `001768a`, includes the git/dev-tooling lexicon, PR #1 merged), `dotfiles project` (setup/list/status/pr/doctor), `gh`. Source: `Cargo.toml`, `src/`.
- Replaced an earlier bash+bats version the same day (same behaviour, now delegating scaffold to `dotfiles project setup`).
- Cargo uses the git CLI to fetch Actname over ssh (`.cargo/config.toml`).
- Not verified: a real `repo new` without `--dry-run` (creates a GitHub repo).
- Directory still named `repo` (placeholder); not pushed to GitHub.
