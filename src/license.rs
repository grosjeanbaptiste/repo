//! The licence of a project tree and the `License:` doctor line
//! (port of dotfiles `scripts/lib/project-license.sh`).
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    None,
    Proprietary,
    Open(&'static str),
    Other,
}

/// Names GitHub reads, in order.
const NAMES: [&str; 5] = ["LICENSE", "LICENSE.md", "LICENSE.txt", "LICENCE", "COPYING"];

/// Phrase carried by each licence family; first match wins, so the more
/// specific GNU families come before the plain GPL and `All rights reserved`
/// comes last (a grant beside it makes the licence open).
// ponytail: a licence worded otherwise reads as `Other`, to be read by a person.
const FAMILIES: [(&str, &str); 9] = [
    ("Permission is hereby granted", "MIT"),
    ("Apache License", "Apache-2.0"),
    ("Redistribution and use in source and binary forms", "BSD"),
    ("GNU AFFERO GENERAL PUBLIC", "AGPL"),
    ("GNU LESSER GENERAL PUBLIC", "LGPL"),
    ("GNU GENERAL PUBLIC", "GPL"),
    ("Mozilla Public License", "MPL"),
    ("Permission to use, copy, modify, and/or distribute", "ISC"),
    ("free and unencumbered", "Unlicense"),
];

pub fn file(dir: &Path) -> Option<PathBuf> {
    NAMES.iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

pub fn kind(dir: &Path) -> Result<Kind> {
    let Some(path) = file(dir) else {
        return Ok(Kind::None); // no licence file is an answer, not a failure
    };
    let bytes = fs::read(&path)
        .with_context(|| format!("project license: cannot read {}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    if let Some((_, family)) = FAMILIES.iter().find(|(phrase, _)| text.contains(phrase)) {
        return Ok(Kind::Open(family));
    }
    Ok(if text.contains("All rights reserved") {
        Kind::Proprietary
    } else {
        Kind::Other
    })
}

pub fn doctor_line(dir: &Path) -> Result<String> {
    let kind = kind(dir)?;
    let name = file(dir)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let label = "License:";
    Ok(match kind {
        Kind::None => format!(
            "    {label:<22} NONE — every right is reserved, but nothing says so: `dotfiles project apply` deploys the proprietary LICENSE"
        ),
        Kind::Proprietary => format!("    {label:<22} proprietary ({name})"),
        Kind::Open(family) => format!(
            "    {label:<22} open ({family}, {name}) — not the strict default; changing it withdraws rights, the owner's call"
        ),
        Kind::Other => format!(
            "    {label:<22} its own words ({name}) — read it: is it as strict as the proprietary default?"
        ),
    })
}

#[cfg(test)]
mod tests {
    include!("license_tests.rs");
}
