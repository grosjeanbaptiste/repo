# repo

Manage git repositories by reusing what exists:

- **actname** (Rust library, pinned by commit) names the repository.
- **`repo` itself** scaffolds it (`src/scaffold/`: mkdir, git init, templates from `~/dotfiles/project-templates`, commit, private GitHub repo via `gh`); `--via-dotfiles` uses the old `dotfiles project setup` instead.
- **`dotfiles project`** still handles `deploy`; they move into `repo` one by one (see `CONTEXT.md`).
- **Prerequisites:** `git` and `gh` on `PATH`; templates are read from `~/dotfiles/project-templates` (`PROJECT_REPO_ROOT` / `DOTFILES_REPO_ROOT` override the root).
- **actit** (Rust library, pinned by commit) checks that the name is free on GitHub, under any owner, through `gh`.

## Install

Needs Rust 1.85 or newer, SSH access to GitHub (the actname and actit libraries are
fetched over ssh), `gh` authenticated (`gh auth login`) and `dotfiles` on `PATH`.

```bash
CARGO_NET_GIT_FETCH_WITH_CLI=true \
  cargo install --locked --git https://github.com/grosjeanbaptiste/repo
export PATH="$HOME/.cargo/bin:$PATH"
repo new package/rust --prefix act --domain "git repositories tooling" --dry-run
```

From a checkout: `cargo install --locked --path .` (`.cargo/config.toml` already sets
the git fetch). The binary is `~/.cargo/bin/repo`; update by re-running the first
command with `--force`.

## Usage

```bash
repo new package/rust --prefix act --domain "git repositories tooling" [--root ~/Source] [--no-push] [--dry-run]
repo new package/rust --name ifdck --no-push   # exact name, still checked
repo check [path] [--type t]                    # audit a project against the templates
repo apply [path] [--type t] [--force|--diff]   # drop/refresh the standard files
repo license [path]                             # none | proprietary | open | other
repo list | scan [ROOT...] | status [NAME]      # machine-wide project registry ($DOTFILES_STATE_ROOT/projects.txt)
repo rename-branch [PATH] [--from master] [--to main] [--dry-run] [--delete-old]   # default branch, local + GitHub
repo push [--dir PATH] [--dry-run] [--yes]      # push every nested repo that is clean and ahead; asks first
repo pr create [--base B] [--title T] [--body D]    # PR for the current branch; title/description asked on stdin if not given
repo pr merge [N] [--yes]                           # merge commit, branch deleted, base fast-forwarded locally
repo workspace list | status | push [-n]            # folders of the nearest *.code-workspace ($DOTFILES_WORKSPACE_FILE overrides)
repo workspace add NAME [--clone URL | --init TYPE] # adopt ./NAME, or clone / scaffold it first
repo workspace remove NAME                          # forget the folder; the directory stays
repo install NAME [--package P] [--bin B] [--force]   # cargo install --path of a registered, clean, committed project, through the heavy queue
repo release [--require-audit] [--path DIR] [--script PATH] [ARGS...]   # run the project's release script with the profile and .envrc loaded
repo run NAME-OR-DIR [VERB [ARGS...]]               # run a Makefile target, melos script or cargo/flutter command; no VERB lists them
repo doctor [PATH]                                 # one-glance health: git, open PRs and CI (gh via actautoma), TODO/FIXME, licence, audit
repo audit [PATH]                                  # env vars / CI secrets a project references, and where each is fulfilled
repo doctor | …                                     # forwarded to `dotfiles project`
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
