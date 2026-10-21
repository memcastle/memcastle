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

fn release_version(tag: &str) -> Option<semver::Version> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    // Development and build-metadata tags are not publisher releases.
    if version.split('.').count() != 3
        || !version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    semver::Version::parse(version).ok()
}

/// Use the exact release tag at HEAD, or an identifiable SemVer development build.
///
/// # Errors
///
/// A checkout without Git history needs an explicit `--version` when packaging.
pub fn infer(root: &Path) -> Result<String> {
    let hash = git(root, &["rev-parse", "--short=12", "HEAD"])?;
    let dirty = !git(root, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    // Git globs cannot express SemVer, so only pass validated tag names to describe.
    let tags = git(root, &["tag", "--merged", "HEAD", "--list"])?;
    let release_tags: Vec<_> = tags
        .lines()
        .filter(|tag| release_version(tag).is_some())
        .collect();
    if !release_tags.is_empty() {
        let top = git(root, &["rev-parse", "--show-toplevel"])?;
        let top = Path::new(&top)
            .canonicalize()
            .map_err(|e| invalid(format!("cannot resolve Git root: {e}")))?;
        let root = root
            .canonicalize()
            .map_err(|e| invalid(format!("cannot resolve plugin root: {e}")))?;
        if root != top {
            return Err(invalid(
                "plain release tags belong to the whole Git repository; package a plugin in a subdirectory with --version",
            ));
        }
    }
    let mut args = vec!["describe", "--tags", "--long", "--abbrev=12"];
    for tag in &release_tags {
        args.extend(["--match", *tag]);
    }
    let described = if release_tags.is_empty() {
        None
    } else {
        Some(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(&args)
                .output()
                .map_err(|e| invalid(format!("cannot read plugin tags: {e}")))?,
        )
    };
    let selected = described.as_ref().filter(|output| output.status.success());
    let (version, distance) = if let Some(described) = selected {
        let described = String::from_utf8_lossy(&described.stdout);
        let (tag_and_distance, _) = described
            .trim()
            .rsplit_once("-g")
            .ok_or_else(|| invalid("Git returned an invalid plugin tag description"))?;
        let (tag, distance) = tag_and_distance
            .rsplit_once('-')
            .ok_or_else(|| invalid("Git returned an invalid plugin tag distance"))?;
        let parsed = release_version(tag)
            .ok_or_else(|| invalid(format!("invalid plugin release tag `{tag}`")))?;
        // Two names for one version are fine; different versions on a commit are ambiguous.
        let commit = git(root, &["rev-list", "-n", "1", tag])?;
        let aliases = git(root, &["tag", "--points-at", &commit, "--list"])?;
        if aliases
            .lines()
            .filter_map(release_version)
            .any(|candidate| candidate != parsed)
        {
            return Err(invalid(format!(
                "conflicting plugin release tags on commit {commit}"
            )));
        }
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
    if selected.is_some() && distance == 0 && !dirty {
        return Ok(version.to_string());
    }
    let next = if selected.is_some() {
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
    fn plain_and_v_prefixed_release_tags_keep_distinct_release_and_development_versions() {
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
        let initial = infer(root.path()).unwrap();
        assert!(initial.starts_with("0.1.0-dev.1+g"), "{initial}");
        run(root.path(), &["tag", "0.1.0"]);
        assert_eq!(infer(root.path()).unwrap(), "0.1.0");
        std::fs::write(root.path().join("plugin.toml"), "next").unwrap();
        assert!(infer(root.path()).unwrap().contains(".d20"));
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
        run(root.path(), &["tag", "vnot-a-version"]);
        run(root.path(), &["tag", "0.1.1-rc.1"]);
        run(root.path(), &["tag", "v0.1.1+build.5"]);
        let ahead = infer(root.path()).unwrap();
        assert!(ahead.starts_with("0.1.1-dev.1+g"), "{ahead}");
        run(root.path(), &["tag", "v0.1.1"]);
        assert_eq!(infer(root.path()).unwrap(), "0.1.1");
        run(root.path(), &["tag", "0.1.1"]);
        assert_eq!(infer(root.path()).unwrap(), "0.1.1");
        std::fs::write(root.path().join("plugin.toml"), "third").unwrap();
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
                "third",
            ],
        );
        let after_release = infer(root.path()).unwrap();
        assert!(
            after_release.starts_with("0.1.2-dev.1+g"),
            "{after_release}"
        );
    }

    #[test]
    fn conflicting_versions_on_one_commit_are_rejected() {
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
        run(root.path(), &["tag", "0.1.0"]);
        run(root.path(), &["tag", "v0.2.0"]);
        assert!(
            infer(root.path())
                .unwrap_err()
                .to_string()
                .contains("conflicting plugin release tags")
        );
    }

    #[test]
    fn a_nested_plugin_must_supply_its_own_version_when_the_repository_has_release_tags() {
        let root = tempfile::tempdir().unwrap();
        run(root.path(), &["init", "-q"]);
        std::fs::create_dir(root.path().join("plugins")).unwrap();
        std::fs::write(root.path().join("plugins/plugin.toml"), "first").unwrap();
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
        run(root.path(), &["tag", "0.1.0"]);
        assert!(
            infer(&root.path().join("plugins"))
                .unwrap_err()
                .to_string()
                .contains("--version")
        );
    }

    #[test]
    fn a_project_without_git_history_needs_an_explicit_version() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            infer(root.path())
                .unwrap_err()
                .to_string()
                .contains("--version")
        );
    }
}
