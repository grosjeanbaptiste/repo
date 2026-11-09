# repo

Manage git repositories by reusing what exists:

- **actname** (Rust library, pinned by commit) names the repository.
- **`dotfiles project`** scaffolds it (mkdir, git init, template, commit, GitHub) and handles `list`, `status`, `pr`, `doctor`, …
- **actit** (Rust library, pinned by commit) checks that the name is free on GitHub, under any owner, through `gh`.

```bash
repo new package/rust --prefix act --domain "git repositories tooling" [--root ~/Source] [--no-push] [--dry-run]
repo new package/rust --name ifdck --no-push   # exact name, still checked
repo status | list | pr create | doctor   # forwarded to `dotfiles project`
```

Names whose directory exists, or that any GitHub owner already uses, are skipped.

## Checks

```bash
cargo test && cargo clippy --all-targets -- -D warnings
dotfiles heavy run --medium -- cargo test --test live -- --ignored   # real pipeline for ifdck, no GitHub write
```

Coverage of `repo new`: 36 unit tests (fakes), 10 CLI tests (no network), 17 live tests.
Cases covered: every project type; free, taken, invalid names (GitHub rules: 1 to 100 characters
of `A-Z a-z 0-9 . _ -`, not `.`/`..`, no `.git` suffix; unicode and symbols refused); `..`, `a/b`
and other non-single-component names; a directory, file or dangling symlink already there; missing
root, file as root, relative root, `~`, `~user`, missing `HOME`; actname refusals (prefix, length,
domain, limit); GitHub failing at once or half way (rate limit), invalid token, offline; missing
`gh` or `dotfiles`; unknown project type (no partial scaffold); two simultaneous runs (one wins);
forwarding of `dotfiles project` commands, success and failure.

`src/pipeline.rs` is the whole `repo new` flow with two injected seams (GitHub, `dotfiles`);
its tests run it for `ifdck` end to end with fakes. `tests/live.rs` runs the same flow for real
into a temp directory (`--no-push`: GitHub is only read).
