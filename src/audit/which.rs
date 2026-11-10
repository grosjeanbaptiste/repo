//! `command -v`: is an executable of that name in the `PATH` directories.
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;

pub fn on_path(name: &str, path: Option<&OsStr>) -> bool {
    let Some(path) = path else { return false };
    std::env::split_paths(path)
        .filter(|d| !d.as_os_str().is_empty())
        .any(|d| {
            std::fs::metadata(d.join(name))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
        r.unwrap_or_else(|e| panic!("{e}"))
    }

    fn tool(dir: &std::path::Path, name: &str, mode: u32) {
        ok(fs::write(dir.join(name), "#!/bin/sh\n"));
        ok(fs::set_permissions(
            dir.join(name),
            fs::Permissions::from_mode(mode),
        ));
    }

    #[test]
    fn an_executable_in_a_path_directory_is_found() {
        let d = ok(tempfile::tempdir());
        tool(d.path(), "mytool", 0o755);
        assert!(on_path("mytool", Some(d.path().as_os_str())));
    }

    #[test]
    fn the_first_directory_may_miss_and_a_later_one_hit() {
        let (a, b) = (ok(tempfile::tempdir()), ok(tempfile::tempdir()));
        tool(b.path(), "mytool", 0o755);
        let path = ok(std::env::join_paths([a.path(), b.path()]));
        assert!(on_path("mytool", Some(&path)));
    }

    #[test]
    fn a_non_executable_file_a_directory_and_a_missing_name_are_not_found() {
        let d = ok(tempfile::tempdir());
        tool(d.path(), "plain", 0o644);
        ok(fs::create_dir(d.path().join("adir")));
        let p = Some(d.path().as_os_str());
        assert!(!on_path("plain", p));
        assert!(!on_path("adir", p));
        assert!(!on_path("nope", p));
    }

    #[test]
    fn no_path_variable_finds_nothing() {
        assert!(!on_path("sh", None));
        assert!(!on_path("sh", Some(OsStr::new(""))));
    }
}
