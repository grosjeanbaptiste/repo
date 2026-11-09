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

Coverage of `repo new`: 25 unit tests (fakes), 4 CLI tests (no network), 10 live tests
(every project type, taken name, run twice, unknown type, missing `gh`, missing `dotfiles`,
generated name, unknown domain, forwarding). A name that is not one path component (`..`, `a/b`)
is refused as invalid before any filesystem or network call.

`src/pipeline.rs` is the whole `repo new` flow with two injected seams (GitHub, `dotfiles`);
its tests run it for `ifdck` end to end with fakes. `tests/live.rs` runs the same flow for real
into a temp directory (`--no-push`: GitHub is only read).
