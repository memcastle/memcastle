//! Generic plugin authoring works without a configured daemon or provider checkout.

use std::path::Path;

use assert_cmd::cargo::cargo_bin;

const EMPTY_PLUGIN: &str = "format = 1\nmemcastle = '>=0.4'\n[plugin]\nid = 'example'\nversion = '0.1.0'\nprovider = 'example'\ndescription = 'example'\nrepository = 'https://github.com/example/example'\nlicense = 'MIT'\n";

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
    let inspected = command(root.path(), &["inspect", "plugin.toml"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(manifest["plugin"]["id"], "example");
    assert_eq!(manifest["modules"].as_array().unwrap().len(), 0);

    let first = command(root.path(), &["package", "plugin.toml"]);
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
fn a_plugin_is_installed_and_tested_in_disposable_state() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("plugin.toml"), EMPTY_PLUGIN).unwrap();
    assert!(
        command(root.path(), &["package", "plugin.toml"])
            .status
            .success()
    );
    let checked = command(root.path(), &["test", "dist/example-0.1.0.tar.gz"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert!(!root.path().join("palace").exists());
}
