//! Environment variables and the home folder, snapshotted so tests never read the real ones.
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Vars {
    map: HashMap<String, String>,
    pub home: PathBuf,
}

impl Vars {
    #[cfg(test)]
    pub fn of(pairs: &[(&str, &str)], home: &std::path::Path) -> Vars {
        let map = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Vars {
            map,
            home: home.to_path_buf(),
        }
    }

    /// The process environment; `HOME` must be set.
    pub fn from_process() -> Result<Vars> {
        let home = std::env::var("HOME")
            .ok()
            .filter(|h| !h.is_empty())
            .context("HOME is not set")?;
        Ok(Vars {
            map: std::env::vars().collect(),
            home: PathBuf::from(home),
        })
    }

    /// Like the shell's `${NAME:-}`: an empty value counts as unset.
    pub fn var(&self, name: &str) -> Option<String> {
        self.map.get(name).cloned().filter(|v| !v.is_empty())
    }

    /// A variable holding a path, or `default` under the home folder.
    pub fn path_or(&self, name: &str, default: &str) -> PathBuf {
        self.var(name)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home.join(default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_counts_as_unset_and_path_or_falls_back_under_home() {
        let v = Vars::of(&[("A", "x"), ("E", "")], std::path::Path::new("/h"));
        assert_eq!(v.var("A").as_deref(), Some("x"));
        assert_eq!((v.var("E"), v.var("Z")), (None, None));
        assert_eq!(v.path_or("A", "d"), PathBuf::from("x"));
        assert_eq!(v.path_or("E", "d/e"), PathBuf::from("/h/d/e"));
    }
}
