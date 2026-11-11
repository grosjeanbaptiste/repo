//! The directory of one workspace member, read from `cargo metadata`.
use anyhow::{Context, Result, anyhow, bail};
use std::path::{Path, PathBuf};

/// `cargo install --path` only sees the root package: a member is installed from its own directory.
pub fn member_dir(metadata: &str, package: &str, project: &Path) -> Result<PathBuf> {
    let json: serde_json::Value =
        serde_json::from_str(metadata).context("reading cargo metadata")?;
    let packages = json["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata has no packages list"))?;
    let Some(found) = packages.iter().find(|p| p["name"] == package) else {
        bail!("no package {package} in {}", project.display());
    };
    let manifest = found["manifest_path"]
        .as_str()
        .ok_or_else(|| anyhow!("cargo metadata: package {package} has no manifest_path"))?;
    Path::new(manifest)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            anyhow!("cargo metadata: manifest_path of {package} has no directory: {manifest}")
        })
}

#[cfg(test)]
mod tests {
    include!("manifest_tests.rs");
}
