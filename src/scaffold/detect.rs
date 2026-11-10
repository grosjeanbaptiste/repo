//! Guess the project type of an existing directory (`workspace` first).
use std::fs;
use std::path::Path;

fn has_ext(dir: &Path, ext: &str) -> bool {
    fs::read_dir(dir).is_ok_and(|rd| {
        rd.filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == ext))
    })
}

pub fn detect(dir: &Path) -> Option<&'static str> {
    let has = |f: &str| dir.join(f).exists();
    if has_ext(dir, "code-workspace") {
        Some("workspace")
    } else if dir.join("stores/ios").is_dir() && dir.join("stores/android").is_dir() {
        Some("mobile")
    } else if has_ext(dir, "tex") {
        Some("tex")
    } else if has("pubspec.yaml") {
        Some("flutter")
    } else if has("pyproject.toml") || has("setup.py") {
        Some("python")
    } else if has("package.json") {
        Some("node")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[&str]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        for f in files {
            let p = d.path().join(f);
            fs::create_dir_all(p.parent().unwrap_or(d.path())).unwrap_or_else(|e| panic!("{e}"));
            fs::write(p, "").unwrap_or_else(|e| panic!("{e}"));
        }
        d
    }

    #[test]
    fn each_marker_maps_to_its_type() {
        for (files, want) in [
            (&["a.code-workspace"][..], Some("workspace")),
            (&["stores/ios/x", "stores/android/x"], Some("mobile")),
            (&["main.tex"], Some("tex")),
            (&["pubspec.yaml"], Some("flutter")),
            (&["pyproject.toml"], Some("python")),
            (&["setup.py"], Some("python")),
            (&["package.json"], Some("node")),
            (&["README.md"], None),
            (&[], None),
        ] {
            assert_eq!(detect(dir_with(files).path()), want, "{files:?}");
        }
    }

    #[test]
    fn workspace_beats_everything_and_mobile_needs_both_stores() {
        assert_eq!(
            detect(dir_with(&["w.code-workspace", "main.tex", "package.json"]).path()),
            Some("workspace")
        );
        assert_eq!(
            detect(dir_with(&["stores/ios/x", "package.json"]).path()),
            Some("node")
        );
        assert_eq!(
            detect(dir_with(&["stores/ios/x", "stores/android/x", "main.tex"]).path()),
            Some("mobile")
        );
    }

    #[test]
    fn missing_directory_detects_nothing() {
        assert_eq!(detect(Path::new("/nonexistent/zz")), None);
    }
}
