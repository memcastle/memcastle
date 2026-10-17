//! Derive a publisher's development version from its own Git release tags.

use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};

fn invalid(message: impl Into<String>) -> Error {
    Error::PluginPackageInvalid {
        message: message.into(),
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| invalid(format!("cannot run Git for plugin version: {e}")))?;
    if !output.status.success() {
        return Err(invalid(format!(
            "cannot infer plugin version from Git: {}; supply --version explicitly",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Use the exact release tag at HEAD, or an identifiable SemVer development build.
///
/// # Errors
///
/// A checkout without Git history needs an explicit `--version` when packaging.
pub fn infer(root: &Path, plugin_id: &str) -> Result<String> {
    let hash = git(root, &["rev-parse", "--short=12", "HEAD"])?;
    let dirty = !git(root, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    let described = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "describe",
            "--tags",
            "--long",
            "--abbrev=12",
            "--match",
            &format!("{plugin_id}/v[0-9]*"),
        ])
        .output()
        .map_err(|e| invalid(format!("cannot read plugin tags: {e}")))?;
    let (version, distance) = if described.status.success() {
        let described = String::from_utf8_lossy(&described.stdout);
        let (tag_and_distance, _) = described
            .trim()
            .rsplit_once("-g")
            .ok_or_else(|| invalid("Git returned an invalid plugin tag description"))?;
        let (tag, distance) = tag_and_distance
            .rsplit_once('-')
            .ok_or_else(|| invalid("Git returned an invalid plugin tag distance"))?;
        let version = tag
            .strip_prefix(&format!("{plugin_id}/v"))
            .ok_or_else(|| invalid("Git selected a tag belonging to another plugin"))?;
        let parsed = semver::Version::parse(version)
            .map_err(|e| invalid(format!("invalid plugin release tag `{tag}`: {e}")))?;
        let distance = distance
            .parse::<u64>()
            .map_err(|_| invalid("invalid plugin tag distance"))?;
        (parsed, distance)
    } else {
        let distance = git(root, &["rev-list", "--count", "HEAD"])?
            .parse::<u64>()
            .map_err(|_| invalid("invalid Git commit count"))?;
        (semver::Version::new(0, 1, 0), distance)
    };
    if described.status.success() && distance == 0 && !dirty {
        return Ok(version.to_string());
    }
    let next = if described.status.success() {
        let patch = version
            .patch
            .checked_add(1)
            .ok_or_else(|| invalid("plugin release patch version overflow"))?;
        semver::Version::new(version.major, version.minor, patch)
    } else {
        version
    };
    let marker = if dirty {
        format!(".d{}", chrono::Utc::now().format("%Y%m%d"))
    } else {
        String::new()
    };
    Ok(format!("{next}-dev.{distance}+g{hash}{marker}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn an_exact_plugin_tag_and_later_commits_have_distinct_versions() {
        let root = tempfile::tempdir().unwrap();
        run(root.path(), &["init", "-q"]);
        std::fs::write(root.path().join("plugin.toml"), "first").unwrap();
        run(root.path(), &["add", "."]);
        run(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "first",
            ],
        );
        let initial = infer(root.path(), "example").unwrap();
        assert!(initial.starts_with("0.1.0-dev.1+g"), "{initial}");
        run(root.path(), &["tag", "example/v0.1.0"]);
        assert_eq!(infer(root.path(), "example").unwrap(), "0.1.0");
        std::fs::write(root.path().join("plugin.toml"), "next").unwrap();
        assert!(infer(root.path(), "example").unwrap().contains(".d20"));
        run(root.path(), &["add", "."]);
        run(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "next",
            ],
        );
        run(root.path(), &["tag", "other/v9.0.0"]);
        let ahead = infer(root.path(), "example").unwrap();
        assert!(ahead.starts_with("0.1.1-dev.1+g"), "{ahead}");
    }

    #[test]
    fn a_project_without_git_history_needs_an_explicit_version() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            infer(root.path(), "example")
                .unwrap_err()
                .to_string()
                .contains("--version")
        );
    }
}
