use super::super::fake::Fake;
use super::super::{Level, ok as okf, warn};
use super::*;
use std::fs;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn put(root: &Path, rel: &str, text: &str) {
    ok(fs::write(root.join(rel), text));
}

fn dir() -> tempfile::TempDir {
    ok(tempfile::tempdir())
}

#[test]
fn tool_versions_counts_the_tool_lines() {
    let d = dir();
    put(d.path(), ".tool-versions", "nodejs 20\n\n  python 3.12\n# comment\n1bad\n");
    let f = Fake::new(&["mise"]);
    let has = f.has();
    assert_eq!(
        ok(tool_versions(d.path(), &f.host(&has))),
        Some(okf(".tool-versions present (2 tools) — `mise install` to provision"))
    );
}

#[test]
fn tool_versions_prefers_mise_then_asdf_then_warns() {
    let d = dir();
    put(d.path(), ".tool-versions", "nodejs 20\n");
    let table = [
        (vec!["mise", "asdf"], okf(".tool-versions present (1 tools) — `mise install` to provision")),
        (vec!["asdf"], okf(".tool-versions present (1 tools) — `asdf install` to provision")),
        (vec![], warn(".tool-versions present (1 tools) but neither mise nor asdf installed (suggest: `dotfiles add mise`)")),
    ];
    for (tools, want) in table {
        let f = Fake::new(&tools);
        let has = f.has();
        assert_eq!(ok(tool_versions(d.path(), &f.host(&has))), Some(want));
    }
}

#[test]
fn an_empty_tool_versions_counts_zero_not_a_doubled_zero() {
    let d = dir();
    put(d.path(), ".tool-versions", "");
    let f = Fake::new(&["mise"]);
    let has = f.has();
    let r = ok(tool_versions(d.path(), &f.host(&has)));
    assert_eq!(r, Some(okf(".tool-versions present (0 tools) — `mise install` to provision")));
}

#[test]
fn tool_versions_absent_is_silent() {
    let (d, f) = (dir(), Fake::new(&["mise"]));
    let has = f.has();
    assert_eq!(ok(tool_versions(d.path(), &f.host(&has))), None);
}

fn nvm(file: &str, node: Option<&'static str>) -> Option<super::super::Finding> {
    let d = dir();
    put(d.path(), ".nvmrc", file);
    let f = match node {
        Some(v) => Fake::new(&["node"]).answering("node --version", Ok(v)),
        None => Fake::new(&[]),
    };
    let has = f.has();
    ok(nvmrc(d.path(), &f.host(&has)))
}

#[test]
fn nvmrc_without_node_warns() {
    assert_eq!(nvm("20\n", None), Some(warn(".nvmrc wants node 20 but node not installed")));
}

#[test]
fn nvmrc_matches_on_whole_version_components() {
    assert_eq!(nvm("v20\n", Some("v20.11.1\n")), Some(okf("node 20.11.1 matches .nvmrc")));
    assert_eq!(nvm("20.11.1", Some("v20.11.1")), Some(okf("node 20.11.1 matches .nvmrc")));
    assert_eq!(nvm(" 20.11 \n", Some("v20.11.1")), Some(okf("node 20.11.1 matches .nvmrc")));
}

#[test]
fn nvmrc_does_not_match_a_longer_number_or_another_version() {
    let w = |want: &str, have: &str| warn(format!("node {have} in PATH but .nvmrc wants {want} (suggest: `nvm use`)"));
    assert_eq!(nvm("2\n", Some("v20.1.0")), Some(w("2", "20.1.0")));
    assert_eq!(nvm("18\n", Some("v20.1.0")), Some(w("18", "20.1.0")));
    assert_eq!(nvm("lts/iron\n", Some("v20.1.0")), Some(w("lts/iron", "20.1.0")));
}

#[test]
fn an_empty_nvmrc_is_a_warning() {
    assert_eq!(nvm("\n", Some("v20.1.0")), Some(warn(".nvmrc is empty")));
}

#[test]
fn a_failing_node_version_is_a_failure() {
    let d = dir();
    put(d.path(), ".nvmrc", "20");
    let f = Fake::new(&["node"]).answering("node --version", Err("dyld error"));
    let has = f.has();
    let r = ok(nvmrc(d.path(), &f.host(&has)));
    assert!(matches!(&r, Some(x) if x.level == Level::Fail && x.msg.contains("dyld error")), "{r:?}");
}

fn py(file: &str, tools: &[&'static str], answers: &[(&'static str, &'static str)]) -> Option<super::super::Finding> {
    let d = dir();
    put(d.path(), ".python-version", file);
    let mut f = Fake::new(tools);
    for (p, o) in answers {
        f = f.answering(p, Ok(o));
    }
    let has = f.has();
    ok(python_version(d.path(), &f.host(&has)))
}

#[test]
fn python_missing_warns() {
    assert_eq!(py("3.12\n", &[], &[]), Some(warn(".python-version wants 3.12 but no python in PATH")));
}

#[test]
fn python_prefers_python3_and_strips_the_banner() {
    let r = py("3.12\n", &["python3", "python"], &[("python3 --version", "Python 3.12.4\n")]);
    assert_eq!(r, Some(okf("python 3.12.4 matches .python-version")));
}

#[test]
fn python_falls_back_to_python() {
    let r = py("3.12.4", &["python"], &[("python --version", "Python 3.12.4\n")]);
    assert_eq!(r, Some(okf("python 3.12.4 matches .python-version")));
}

#[test]
fn python_mismatch_warns_with_both_versions() {
    let r = py("3.11\n", &["python3"], &[("python3 --version", "Python 3.12.4\n")]);
    assert_eq!(
        r,
        Some(warn("python 3.12.4 in PATH but .python-version wants 3.11 (suggest: `pyenv install` or `mise use`)"))
    );
    assert_eq!(py("3.1", &["python3"], &[("python3 --version", "Python 3.12.4")]).map(|f| f.level), Some(Level::Warn));
}

#[test]
fn python_version_absent_is_silent() {
    let (d, f) = (dir(), Fake::new(&["python3"]));
    let has = f.has();
    assert_eq!(ok(python_version(d.path(), &f.host(&has))), None);
}

#[test]
fn a_failing_python_version_is_a_failure() {
    let d = dir();
    put(d.path(), ".python-version", "3.12");
    let f = Fake::new(&["python3"]).answering("python3 --version", Err("broken shim"));
    let has = f.has();
    let r = ok(python_version(d.path(), &f.host(&has)));
    assert!(matches!(&r, Some(x) if x.level == Level::Fail && x.msg.contains("broken shim")), "{r:?}");
}

#[test]
fn mise_toml_in_either_name() {
    for name in ["mise.toml", ".mise.toml"] {
        let d = dir();
        put(d.path(), name, "");
        let (with, without) = (Fake::new(&["mise"]), Fake::new(&[]));
        let (h1, h2) = (with.has(), without.has());
        assert_eq!(mise_toml(d.path(), &with.host(&h1)), Some(okf("mise.toml present — `mise install` to provision")));
        assert_eq!(
            mise_toml(d.path(), &without.host(&h2)),
            Some(warn("mise.toml present but mise not installed (suggest: `dotfiles add mise`)"))
        );
    }
    let (d, f) = (dir(), Fake::new(&[]));
    let has = f.has();
    assert_eq!(mise_toml(d.path(), &f.host(&has)), None);
}

#[test]
fn every_lockfile_maps_to_its_tool_in_order() {
    let table = [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lockb", "bun"),
        ("bun.lock", "bun"),
        ("package-lock.json", "npm"),
        ("poetry.lock", "poetry"),
        ("uv.lock", "uv"),
        ("Pipfile.lock", "pipenv"),
        ("Gemfile.lock", "bundle"),
        ("Cargo.lock", "cargo"),
        ("go.sum", "go"),
    ];
    for (lock, tool) in table {
        let d = dir();
        put(d.path(), lock, "");
        let (with, without) = (Fake::new(&[tool]), Fake::new(&[]));
        let (h1, h2) = (with.has(), without.has());
        assert_eq!(package_managers(d.path(), &with.host(&h1)), [okf(format!("{tool} available (declared via {lock})"))], "{lock}");
        assert_eq!(
            package_managers(d.path(), &without.host(&h2)),
            [warn(format!("{tool} declared via {lock} but not installed (suggest: `dotfiles add {tool}`)"))],
            "{lock}"
        );
    }
}

#[test]
fn several_lockfiles_give_several_findings_in_the_bash_order() {
    let d = dir();
    for l in ["Cargo.lock", "yarn.lock", "go.sum"] {
        put(d.path(), l, "");
    }
    let f = Fake::new(&["cargo", "yarn", "go"]);
    let has = f.has();
    let msgs: Vec<String> = package_managers(d.path(), &f.host(&has)).into_iter().map(|x| x.msg).collect();
    assert_eq!(msgs, ["yarn available (declared via yarn.lock)", "cargo available (declared via Cargo.lock)", "go available (declared via go.sum)"]);
}

#[test]
fn no_lockfile_no_finding() {
    let (d, f) = (dir(), Fake::new(&["cargo"]));
    let has = f.has();
    assert!(package_managers(d.path(), &f.host(&has)).is_empty());
}

#[test]
fn direnv_without_an_envrc_is_silent() {
    let (d, f) = (dir(), Fake::new(&["direnv"]));
    let has = f.has();
    assert_eq!(direnv(d.path(), &f.host(&has)), None);
}

#[test]
fn direnv_missing_warns() {
    let d = dir();
    put(d.path(), ".envrc", "");
    let f = Fake::new(&[]);
    let has = f.has();
    assert_eq!(direnv(d.path(), &f.host(&has)), Some(warn(".envrc present but direnv not installed (suggest: `dotfiles add direnv`)")));
}

#[test]
fn direnv_reads_the_rc_state() {
    let d = dir();
    put(d.path(), ".envrc", "");
    let table = [
        ("direnv exec path\nFound RC allowed true\n", okf("direnv loaded for this repo")),
        ("Found RC allowed false\n", warn(".envrc present but not allowed (run `direnv allow` inside the repo)")),
        ("Found RC allowed 0\n", warn(".envrc present but not allowed (run `direnv allow` inside the repo)")),
        ("nothing useful\n", warn(".envrc present but direnv reports unknown state")),
    ];
    for (out, want) in table {
        let f = Fake::new(&["direnv"]).answering("direnv status", Ok(out));
        let has = f.has();
        assert_eq!(direnv(d.path(), &f.host(&has)), Some(want), "{out:?}");
    }
}

#[test]
fn a_failing_direnv_status_warns_with_the_reason() {
    let d = dir();
    put(d.path(), ".envrc", "");
    let f = Fake::new(&["direnv"]).answering("direnv status", Err("boom"));
    let has = f.has();
    let r = direnv(d.path(), &f.host(&has));
    assert!(matches!(&r, Some(x) if x.level == Level::Warn && x.msg.contains("unknown state") && x.msg.contains("boom")), "{r:?}");
}
