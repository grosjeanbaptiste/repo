//! Starting the release script in the environment of the profile and the project's `.envrc`.
use anyhow::Result;
use std::path::Path;

/// Sources `$1` (the machine profile) and `$2/.envrc` with everything exported, then replaces itself
/// with the release script. A source that fails is said on stderr, with what it printed; the script still runs.
pub const SNIPPET: &str = r#"
source_quiet() {
  local log status=0
  log=$(mktemp) || { echo "warn: release: cannot create a temporary file to read $1" >&2; return 1; }
  source "$1" 2>"$log" || status=$?
  if [ "$status" -ne 0 ]; then
    echo "warn: release: $1 exited with status $status" >&2
    cat "$log" >&2
  fi
  rm -f "$log"
}
set -a
[ -r "$1" ] && source_quiet "$1"
if [ -r "$2/.envrc" ]; then
  pushd "$2" >/dev/null
  source_quiet "$2/.envrc"
  popd >/dev/null
fi
set +a
shift 2
exec "$@"
"#;

/// The command line that runs `command` (plus `forward`) after loading `profile` and `target/.envrc`.
pub fn argv(profile: &Path, target: &Path, command: &[String], forward: &[String]) -> Vec<String> {
    let mut v = vec![
        "bash".to_string(),
        "-c".into(),
        SNIPPET.into(),
        "release".into(),
    ];
    v.push(profile.to_string_lossy().into_owned());
    v.push(target.to_string_lossy().into_owned());
    v.extend(command.iter().cloned());
    v.extend(forward.iter().cloned());
    v
}

/// Replaces the current process with `argv`; comes back only when that failed.
pub trait Launch {
    fn launch(&self, argv: &[String]) -> Result<()>;
}

#[cfg(test)]
mod tests {
    include!("launch_tests.rs");
}
