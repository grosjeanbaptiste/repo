//! Build with flutter, then publish to the platform's store (port of `project_deploy_native`).
use super::ctx::Ctx;
use super::target::Target;
use super::tool::{Call, plain};
use super::{asc, brew, find, msstore, play, snap};
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

fn flutter_target(target: Target) -> Option<&'static str> {
    match target {
        Target::Ios => Some("ipa"),
        Target::Android => Some("appbundle"),
        Target::Mas | Target::Homebrew => Some("macos"),
        Target::Windows => Some("windows"),
        Target::Linux => Some("linux"),
        Target::Web => None,
    }
}

/// iOS picks the project's `ios/ExportOptions.plist` so the archive is store-signed, not a dev export.
fn flutter_extra(target: Target, root: &Path) -> Vec<String> {
    if target == Target::Ios && root.join("ios/ExportOptions.plist").is_file() {
        return vec![
            "--export-options-plist".into(),
            "ios/ExportOptions.plist".into(),
        ];
    }
    vec![]
}

fn in_root(c: &Ctx, prog: &str, args: &[&str]) -> Result<()> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    c.tool
        .run(&Call {
            cwd: Some(c.root),
            ..plain(prog, &args)
        })
        .with_context(|| format!("{prog} {} failed", args.join(" ")))
}

fn clear(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            Err(e).with_context(|| format!("clearing {}", dir.display()))
        }
        _ => Ok(()), // nothing left over from an earlier build
    }
}

/// A plain `flutter build macos` only yields an unsigned `.app`; the App Store needs a signed installer
/// `.pkg` with the sandbox entitlement, so: signed archive, then export through the manual-signing plist.
fn macos_appstore_pkg(c: &Ctx) -> Result<()> {
    let options = c.root.join("macos/ExportOptions.plist");
    if !options.is_file() {
        bail!(
            "missing macos/ExportOptions.plist under {} (manual app-store signing).",
            c.root.display()
        );
    }
    let archive = c.root.join("build/macos/archive/Runner.xcarchive");
    let pkgdir = c.root.join("build/macos/pkg");
    in_root(
        c,
        "flutter",
        &["build", "macos", "--release", "--config-only"],
    )?;
    clear(&archive)?;
    clear(&pkgdir)?;
    let (archive_s, pkgdir_s, options_s) = (
        archive.display().to_string(),
        pkgdir.display().to_string(),
        options.display().to_string(),
    );
    let build = [
        "-workspace",
        "macos/Runner.xcworkspace",
        "-scheme",
        "Runner",
        "-configuration",
        "Release",
        "-archivePath",
        &archive_s,
        "archive",
        "-allowProvisioningUpdates",
        "-quiet",
    ];
    in_root(c, "xcodebuild", &build)?;
    let export = [
        "-exportArchive",
        "-archivePath",
        &archive_s,
        "-exportPath",
        &pkgdir_s,
        "-exportOptionsPlist",
        &options_s,
        "-allowProvisioningUpdates",
    ];
    in_root(c, "xcodebuild", &export)?;
    if find::artifact(Target::Mas, c.root).is_none() {
        bail!("export produced no .pkg under {pkgdir_s}.");
    }
    Ok(())
}

/// Publishes the already-built artifact to its native channel.
fn publish(c: &mut Ctx, target: Target) -> Result<()> {
    match target {
        Target::Ios | Target::Mas => {
            let Some(artifact) = find::artifact(target, c.root) else {
                bail!(
                    "no {} artifact under {}/build.",
                    target.name(),
                    c.root.display()
                );
            };
            let kind = if target == Target::Ios {
                asc::Kind::Ios
            } else {
                asc::Kind::Macos
            };
            asc::upload(c, kind, &artifact)
        }
        Target::Android => play::upload(c),
        Target::Homebrew => brew::update(c),
        Target::Linux => snap::upload(c),
        Target::Windows => msstore::upload(c),
        Target::Web => bail!("no build recipe for 'web'"),
    }
}

fn plan(c: &mut Ctx, target: Target, flutter: &str) -> Result<()> {
    let root = c.root.display();
    let env = c.env.name();
    let line = if target == Target::Mas {
        format!(
            "macOS App Store: flutter config-only + xcodebuild archive + export signed .pkg (cwd {root}) then upload to {env}"
        )
    } else {
        let extra: String = flutter_extra(target, c.root)
            .iter()
            .map(|a| format!("{a} "))
            .collect();
        format!(
            "flutter build {flutter} --release {extra}(cwd {root}) then publish {} to {env}",
            target.name()
        )
    };
    writeln!(c.out, "{line}").context("writing the plan")
}

/// Builds `target` in the project and publishes it; `dry_run` prints the plan and runs nothing.
pub fn deploy(c: &mut Ctx, target: Target, dry_run: bool) -> Result<()> {
    let Some(flutter) = flutter_target(target) else {
        bail!("no build recipe for '{}'", target.name());
    };
    if dry_run {
        return plan(c, target, flutter);
    }
    if target == Target::Mas {
        macos_appstore_pkg(c)?;
    } else {
        let extra = flutter_extra(target, c.root);
        let mut args = vec!["build", flutter, "--release"];
        args.extend(extra.iter().map(String::as_str));
        in_root(c, "flutter", &args)?;
    }
    publish(c, target)
}

#[cfg(test)]
mod tests {
    include!("native_tests.rs");
}
