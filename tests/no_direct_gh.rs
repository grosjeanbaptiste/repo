//! Rule: `repo` never runs `gh` itself. Forge operations go through actit or actautoma.
//!
//! Ratchet: the files in `DEBT` still call `gh` and are being migrated to actautoma.
//! Any other file that does fails the test; a file in `DEBT` that no longer does fails
//! it too, so the list shrinks and never grows.
use std::fs;
use std::path::Path;

/// Paths under `src/` that still spawn `gh` directly.
const DEBT: [&str; 3] = ["branch/plan.rs", "branch/rename.rs", "scaffold/setup.rs"];

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap_or_else(|e| panic!("{e}")).path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn spawns_gh(text: &str) -> bool {
    [r#"Command::new("gh")"#, r#".run("gh""#, r#"program: "gh""#]
        .iter()
        .any(|p| text.contains(p))
        // `exec.run(\n "gh", ...)` split over lines by rustfmt
        || text.lines().any(|l| l.trim_start().starts_with(r#""gh","#))
}

#[test]
fn only_the_known_debt_spawns_gh_directly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    sources(&root, &mut files);
    assert!(
        !files.is_empty(),
        "found no source file: the guard checks nothing"
    );
    let mut offenders: Vec<String> = files
        .iter()
        .filter(|f| {
            spawns_gh(&fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display())))
        })
        .map(|f| f.strip_prefix(&root).unwrap_or(f).display().to_string())
        .collect();
    offenders.sort();
    assert_eq!(
        offenders, DEBT,
        "repo must not call gh directly: route it through actautoma, and drop fixed files from DEBT"
    );
}
