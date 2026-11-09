# repo

Manage git repositories by reusing what exists:

- **actname** (Rust library, pinned by commit) names the repository.
- **`dotfiles project`** scaffolds it (mkdir, git init, template, commit, GitHub) and handles `list`, `status`, `pr`, `doctor`, …
- **`gh`** checks that the name is free on GitHub.

```bash
repo new package/rust --prefix act --domain "git repositories tooling" [--root ~/Source] [--no-push] [--dry-run]
repo status | list | pr create | doctor   # forwarded to `dotfiles project`
```

Names whose directory or GitHub repo already exist are skipped.

## Checks

```bash
cargo test && cargo clippy --all-targets -- -D warnings
```
