//! Generic plugin authoring works without a configured daemon or provider checkout.

use std::path::Path;

use assert_cmd::cargo::cargo_bin;

const EMPTY_PLUGIN: &str = "format = 1\nmemcastle = '>=0.4'\n[plugin]\nid = 'example'\nprovider = 'example'\ndescription = 'example'\nrepository = 'https://github.com/example/example'\nlicense = 'MIT'\n";
const RELEASE_SEED: &str = "format = 1\nmemcastle = '>=0.4'\n[plugin]\nid = 'example'\nversion = '0.1.0'\nprovider = 'example'\ndescription = 'example'\nrepository = 'https://github.com/example/example'\nlicense = 'MIT'\n";

fn command(root: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new(cargo_bin("memcastle"))
        .arg("--config")
        .arg(root.join("missing.toml"))
        .arg("--json")
        .arg("plugin")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

#[test]
fn packaging_and_inspection_work_offline_and_are_repeatable() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("plugin.toml"), EMPTY_PLUGIN).unwrap();
    std::fs::write(root.path().join("manifest.toml"), RELEASE_SEED).unwrap();
    let inspected = command(root.path(), &["inspect"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(manifest["plugin"]["id"], "example");
    assert_eq!(manifest["plugin"]["version"], "0.1.0");
    assert_eq!(manifest["modules"].as_array().unwrap().len(), 0);

    let first = command(root.path(), &["package"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        Path::new(result["archive"].as_str().unwrap()),
        Path::new("dist").join("example-0.1.0.tar.gz")
    );
    let archive = std::fs::read(root.path().join("dist/example-0.1.0.tar.gz")).unwrap();
    assert_eq!(result["sha256"], memcastle::domain::sha256_hex(&archive));
    assert_eq!(
        std::fs::read_to_string(root.path().join("dist/example-0.1.0.sha256sum")).unwrap(),
        format!(
            "{}  example-0.1.0.tar.gz\n",
            result["sha256"].as_str().unwrap()
        )
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("plugin.toml")).unwrap(),
        EMPTY_PLUGIN
    );
    let generated = memcastle::plugin::package::unpack(&archive).unwrap();
    assert!(generated.manifest_text.contains("version = \"0.1.0\""));
    assert!(
        command(root.path(), &["package", "plugin.toml"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read(root.path().join("dist/example-0.1.0.tar.gz")).unwrap(),
        archive
    );
}

#[test]
fn a_default_manifest_infers_a_plain_tag_from_the_current_repository() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("plugin.toml"), EMPTY_PLUGIN).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "plugin.toml"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "-qm",
        "first",
    ]);
    git(&["tag", "0.1.0"]);

    let inspected = command(root.path(), &["inspect"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(manifest["plugin"]["version"], "0.1.0");

    let packaged = command(root.path(), &["package"]);
    assert!(
        packaged.status.success(),
        "{}",
        String::from_utf8_lossy(&packaged.stderr)
    );
    let archive: serde_json::Value = serde_json::from_slice(&packaged.stdout).unwrap();
    assert_eq!(
        Path::new(archive["archive"].as_str().unwrap()),
        Path::new("dist").join("example-0.1.0.tar.gz")
    );
}

#[test]
fn a_plugin_is_installed_and_tested_in_disposable_state() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("plugin.toml"), EMPTY_PLUGIN).unwrap();
    assert!(
        command(root.path(), &["package", "--version", "0.2.0"])
            .status
            .success()
    );
    assert!(!root.path().join("manifest.toml").exists());
    let checked = command(root.path(), &["test", "dist/example-0.2.0.tar.gz"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert!(!root.path().join("palace").exists());
}

#[test]
fn an_invalid_archive_or_manifest_path_is_rejected_before_any_daemon_starts() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("not-a-plugin.toml"), EMPTY_PLUGIN).unwrap();
    let rejected = command(root.path(), &["package", "not-a-plugin.toml"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("plugin.toml"));
    std::fs::write(root.path().join("bad.tar.gz"), b"not an archive").unwrap();
    let rejected = command(root.path(), &["test", "bad.tar.gz"]);
    assert!(!rejected.status.success());
    assert!(!root.path().join("palace").exists());
}
