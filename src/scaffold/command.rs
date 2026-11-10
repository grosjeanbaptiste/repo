//! `repo check|apply [path]`: the standard-vs-project audit and refresh.
use super::apply::{Mode, sync};
use super::native::{Process, templates_root, year_now};
use super::{
    detect::detect,
    plan::{known_types, layered},
    setup::vars,
};
use anyhow::{Context, Result, bail};
use std::path::Path;

fn resolve(templates: &Path, target: &Path, explicit: Option<&str>) -> Result<String> {
    if let Some(t) = explicit {
        return Ok(t.to_string());
    }
    if let Some(t) = detect(target) {
        return Ok(t.to_string());
    }
    let known = known_types(templates)?.join(", ");
    bail!(
        "Could not detect project type at {}. Pass --type <t>. Known types: {known}",
        target.display()
    )
}

/// Returns the report (header, then one line per file).
pub fn report(
    target: &Path,
    explicit: Option<&str>,
    mode: Mode,
    templates: &Path,
    year: &str,
) -> Result<String> {
    let target = target
        .canonicalize()
        .with_context(|| format!("{}: no such directory", target.display()))?;
    let kind = resolve(templates, &target, explicit)?;
    let name = target
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let v = vars(&Process, &name, year, &target);
    let plan = layered(templates, &kind, mode != Mode::Check)?;
    let head = match mode {
        Mode::Check => format!("==> Checking {kind} project at {}", target.display()),
        _ => format!("==> Applying {kind} standard to {}", target.display()),
    };
    let lines = sync(&target, &plan, &v, mode)?;
    Ok(std::iter::once(head)
        .chain(lines)
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn run(target: &Path, explicit: Option<&str>, mode: Mode) -> Result<()> {
    let templates = templates_root(|k| std::env::var(k).ok())?;
    println!(
        "{}",
        report(target, explicit, mode, &templates, &year_now()?)?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
        r.unwrap_or_else(|e| panic!("{e}"))
    }

    fn templates() -> tempfile::TempDir {
        let d = ok(tempfile::tempdir());
        ok(fs::create_dir_all(d.path().join("common")));
        ok(fs::write(
            d.path().join("common/README.md.tmpl"),
            "# {{project_name}}\n",
        ));
        d
    }

    #[test]
    fn detected_type_drives_the_header_and_check_lists_missing() {
        let (t, p) = (templates(), ok(tempfile::tempdir()));
        ok(fs::write(p.path().join("package.json"), "{}"));
        let r = ok(report(p.path(), None, Mode::Check, t.path(), "2026"));
        assert!(r.starts_with("==> Checking node project at "), "{r}");
        assert!(r.ends_with("  - missing  README.md"), "{r}");
    }

    #[test]
    fn undetectable_type_asks_for_type_and_lists_known_ones() {
        let (t, p) = (templates(), ok(tempfile::tempdir()));
        let e = report(p.path(), None, Mode::Check, t.path(), "2026")
            .err()
            .map(|e| e.to_string());
        assert!(e.is_some_and(
            |m| m.contains("Could not detect project type") && m.contains("Pass --type <t>")
        ));
    }

    #[test]
    fn explicit_type_wins_and_apply_writes_with_the_directory_name() {
        let (t, p) = (templates(), ok(tempfile::tempdir()));
        let r = ok(report(
            p.path(),
            Some("anything"),
            Mode::Apply,
            t.path(),
            "2026",
        ));
        assert!(r.contains("==> Applying anything standard to "));
        let name = ok(p.path().canonicalize())
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        assert_eq!(
            ok(fs::read_to_string(p.path().join("README.md"))),
            format!("# {}\n", name.unwrap_or_default())
        );
    }

    #[test]
    fn check_ignores_the_skeleton_but_apply_seeds_it() {
        let (t, p) = (templates(), ok(tempfile::tempdir()));
        ok(fs::create_dir_all(t.path().join("app/x/skeleton")));
        ok(fs::write(t.path().join("app/x/skeleton/seed.txt"), "s"));
        let c = ok(report(p.path(), Some("app/x"), Mode::Check, t.path(), "1"));
        assert!(!c.contains("seed.txt"), "{c}");
        let a = ok(report(p.path(), Some("app/x"), Mode::Apply, t.path(), "1"));
        assert!(a.contains("  + wrote      seed.txt"), "{a}");
    }

    #[test]
    fn missing_target_fails_loudly() {
        let t = templates();
        assert!(
            report(
                Path::new("/nonexistent/zz"),
                None,
                Mode::Check,
                t.path(),
                "1"
            )
            .is_err()
        );
    }
}
