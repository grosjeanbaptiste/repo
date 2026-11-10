//! The `*.code-workspace` file: where it is and its `folders` array.
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct Folder {
    pub name: String,
    pub path: String,
}

/// First `*.code-workspace` (by name) in `start` or an ancestor; `None` when there is none.
pub fn find(start: &Path) -> Result<Option<PathBuf>> {
    let start = start
        .canonicalize()
        .with_context(|| format!("{}: no such directory", start.display()))?;
    for dir in start.ancestors().filter(|d| *d != Path::new("/")) {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry
                .with_context(|| format!("reading {}", dir.display()))?
                .path();
            let named = path.extension().is_some_and(|e| e == "code-workspace");
            if named && path.is_file() {
                found.push(path);
            }
        }
        found.sort();
        if let Some(first) = found.into_iter().next() {
            return Ok(Some(first));
        }
    }
    Ok(None)
}

fn parse(json: &str) -> Result<Map<String, Value>> {
    match serde_json::from_str(json).context("the workspace is not valid JSON")? {
        Value::Object(map) => Ok(map),
        _ => bail!("the workspace is not a JSON object"),
    }
}

fn entries(root: &Map<String, Value>) -> Result<&[Value]> {
    match root.get("folders") {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => bail!("\"folders\" is not an array"),
    }
}

fn path_of(entry: &Value, i: usize) -> Result<&str> {
    entry
        .get("path")
        .and_then(Value::as_str)
        .with_context(|| format!("folders[{i}] has no string \"path\""))
}

/// `name` is the entry's `name`, else its `path`.
pub fn folders(json: &str) -> Result<Vec<Folder>> {
    let root = parse(json)?;
    entries(&root)?
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let path = path_of(e, i)?.to_string();
            let name = e
                .get("name")
                .and_then(Value::as_str)
                .map_or_else(|| path.clone(), String::from);
            Ok(Folder { name, path })
        })
        .collect()
}

fn text(root: Map<String, Value>) -> Result<String> {
    let mut out =
        serde_json::to_string_pretty(&Value::Object(root)).context("writing the workspace")?;
    out.push('\n');
    Ok(out)
}

/// The text with `{name, path}` appended; `None` when an entry has that path already.
pub fn add(json: &str, name: &str, path: &str) -> Result<Option<String>> {
    let mut root = parse(json)?;
    let mut list = entries(&root)?.to_vec();
    for (i, e) in list.iter().enumerate() {
        if path_of(e, i)? == path {
            return Ok(None);
        }
    }
    list.push(serde_json::json!({ "name": name, "path": path }));
    root.insert("folders".into(), Value::Array(list));
    text(root).map(Some)
}

/// The text without every entry with that path; `None` when there is none.
pub fn remove(json: &str, path: &str) -> Result<Option<String>> {
    let mut root = parse(json)?;
    let list = entries(&root)?;
    let mut kept = Vec::new();
    for (i, e) in list.iter().enumerate() {
        if path_of(e, i)? != path {
            kept.push(e.clone());
        }
    }
    if kept.len() == list.len() {
        return Ok(None);
    }
    root.insert("folders".into(), Value::Array(kept));
    text(root).map(Some)
}

#[cfg(test)]
mod tests {
    include!("file_tests.rs");
}
