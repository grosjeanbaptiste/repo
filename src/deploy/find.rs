//! Built artifacts of a target, found under the project's `build/` (port of `project_deploy_artifact`).
use super::target::Target;
use std::fs;
use std::path::{Path, PathBuf};

/// Entries of `dir` whose name matches `segment` (`*`, `*.ext` or a literal), sorted by name.
fn matching(dir: &Path, segment: &str) -> Vec<PathBuf> {
    if !segment.contains('*') {
        return vec![dir.join(segment)];
    }
    let suffix = segment.trim_start_matches('*');
    let Ok(read) = fs::read_dir(dir) else {
        return vec![];
    }; // a missing folder just has no artifact
    let mut found: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(suffix))
        })
        .collect();
    found.sort();
    found
}

/// The first path (sorted at every level) that exists under `root` following `segments`.
pub fn first(root: &Path, segments: &[&str]) -> Option<PathBuf> {
    let Some((head, rest)) = segments.split_first() else {
        return root.exists().then(|| root.to_path_buf());
    };
    matching(root, head)
        .into_iter()
        .find_map(|p| first(&p, rest))
}

/// The artifact a build of `target` leaves under `root`.
pub fn artifact(target: Target, root: &Path) -> Option<PathBuf> {
    let segments: &[&str] = match target {
        Target::Ios => &["build", "ios", "ipa", "*.ipa"],
        Target::Android => &["build", "app", "outputs", "bundle", "release", "*.aab"],
        Target::Mas => &["build", "macos", "pkg", "*.pkg"],
        Target::Homebrew => &["build", "macos", "Build", "Products", "Release", "*.app"],
        Target::Windows => &["build", "windows", "*", "runner", "Release"],
        Target::Linux => &["build", "linux", "*", "release", "bundle"],
        Target::Web => return None,
    };
    first(root, segments)
}

/// First file called `*.ext` anywhere under `dir`, folders walked in name order.
pub fn deep(dir: &Path, ext: &str) -> Option<PathBuf> {
    let suffix = format!(".{ext}");
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for p in &entries {
        if p.is_file()
            && p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(&suffix))
        {
            return Some(p.clone());
        }
    }
    entries
        .iter()
        .filter(|p| p.is_dir())
        .find_map(|p| deep(p, ext))
}

#[cfg(test)]
mod tests {
    include!("find_tests.rs");
}
