# Context — 2026-11-09

- Rust CLI. Reuses: `actname` lib (git dep pinned to Actname `001768a`, includes the git/dev-tooling lexicon, PR #1 merged), `dotfiles project` (setup/list/status/pr/doctor), `gh`. Source: `Cargo.toml`, `src/`.
- Replaced an earlier bash+bats version the same day (same behaviour, now delegating scaffold to `dotfiles project setup`).
- Cargo uses the git CLI to fetch Actname over ssh (`.cargo/config.toml`).
- Not verified: a real `repo new` without `--dry-run` (creates a GitHub repo).
- Directory still named `repo` (placeholder); not pushed to GitHub.

- 2026-11-09: name availability now comes from the `actit` lib (pinned to Actit `b2044d4`): a name is skipped if its directory exists or if any GitHub owner has a repo of that name. The own `src/github.rs` is gone.
- 2026-11-09: `repo new --name <n>` (exact name, still checked) added; `src/pipeline.rs` holds the flow behind two seams. Pipeline for `ifdck` validated: 8 unit tests with fakes + `tests/live.rs` (ignored by default, run once: pass, 48 s). `ifdck` itself is not created anywhere. Source: `src/pipeline_tests.rs`, `tests/live.rs`.
- 2026-11-09: pipeline tested in every direction before merge: 25 unit + 4 CLI + 10 live (all pass, live run 173 s). Defect found and fixed on the way: `--name ..` was reported as "taken" and only actit stopped `../x`; `naming::taken` now refuses anything that is not one path component first. Source: `src/pipeline_tests.rs`, `tests/cli.rs`, `tests/live.rs`.
- 2026-11-09 (matrix): 36 unit + 10 CLI + 17 live pass (live 273 s). Defects found by the matrix: dangling symlink accepted (fixed, `naming::taken` uses `symlink_metadata`), `~` alone not expanded and missing HOME unnamed (fixed, `src/root.rs`), 101-character and `.git` names accepted (root cause in actit: PR Actit#4, merged; `repo` now pins the merge commit `9b2af77`). Concurrency checked: `dotfiles project setup` refuses "Target already exists", one run wins. Not testable here: a real GitHub push (would create a repository), real rate limit, non-macOS.
