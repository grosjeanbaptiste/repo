//! `workspace list` and `workspace status`: read-only tables.
use super::file::folders;
use super::status::classify;
use crate::scaffold::setup::Exec;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

fn read(ws_file: &Path) -> Result<String> {
    std::fs::read_to_string(ws_file).with_context(|| format!("reading {}", ws_file.display()))
}

/// `home` shortens the path in the empty-workspace message, as `${ws_file#$HOME/}` did.
pub fn list(ws_file: &Path, home: Option<&str>, out: &mut impl Write) -> Result<()> {
    let found = folders(&read(ws_file)?)?;
    if found.is_empty() {
        let shown = home
            .and_then(|h| ws_file.strip_prefix(h).ok())
            .unwrap_or(ws_file);
        writeln!(out, "No folders in {}.", shown.display())?;
        return Ok(());
    }
    writeln!(out, "{:<30} PATH\n{:<30} ----", "NAME", "----")?;
    for f in found {
        writeln!(out, "{:<30} {}", f.name, f.path)?;
    }
    Ok(())
}

pub fn status(ws_file: &Path, probe: &impl Exec, out: &mut impl Write) -> Result<()> {
    let found = folders(&read(ws_file)?)?;
    let dir = ws_file.parent().unwrap_or(Path::new("."));
    writeln!(
        out,
        "{:<25} {:<12} PATH\n{:<25} {:<12} ----",
        "FOLDER", "STATUS", "------", "------"
    )?;
    for f in found {
        let state = classify(probe, &dir.join(&f.path));
        writeln!(out, "{:<25} {:<12} {}", f.name, state.to_string(), f.path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("show_tests.rs");
}
