//! The release script must stop carrying source assets after their providers move out of Core.
//!
//! The same script runs for release packaging and for `mise run sources:package`, so an old output directory
//! must not leak a formerly bundled package into the next release.

#![cfg(unix)]

#[test]
fn the_release_script_removes_stale_bundled_sources_and_archives() {
    let out = tempfile::tempdir().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stale = out.path().join("sources/codex");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("source.wasm"), b"old component").unwrap();
    let archives = out.path().join("archives");
    std::fs::create_dir(&archives).unwrap();
    std::fs::write(archives.join("chatgpt-0.1.0.tar.gz"), b"old archive").unwrap();

    let result = std::process::Command::new("bash")
        .arg(root.join("packaging/sources/build.sh"))
        .arg(assert_cmd::cargo::cargo_bin("memcastle"))
        .arg(out.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        out.path()
            .join("sources")
            .read_dir()
            .unwrap()
            .next()
            .is_none(),
        "provider-owned components must not ship with Core"
    );
    assert!(
        archives.read_dir().unwrap().next().is_none(),
        "an old release archive must not survive packaging"
    );

    let dist = out.path().join("dist");
    std::fs::create_dir(&dist).unwrap();
    let stage = std::process::Command::new("bash")
        .arg(root.join("packaging/sources/stage-archives.sh"))
        .arg(out.path())
        .arg(&dist)
        .output()
        .unwrap();
    assert!(
        stage.status.success(),
        "{}{}",
        String::from_utf8_lossy(&stage.stdout),
        String::from_utf8_lossy(&stage.stderr)
    );
    assert!(dist.read_dir().unwrap().next().is_none());
}

#[test]
fn a_bundled_source_stages_its_archive_and_checksum() {
    let out = tempfile::tempdir().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let archives = out.path().join("archives");
    let dist = out.path().join("dist");
    std::fs::create_dir(&archives).unwrap();
    std::fs::create_dir(&dist).unwrap();
    for name in ["example-1.0.0.tar.gz", "example-1.0.0.tar.gz.sha256"] {
        std::fs::write(archives.join(name), name).unwrap();
    }

    let stage = std::process::Command::new("bash")
        .arg(root.join("packaging/sources/stage-archives.sh"))
        .arg(out.path())
        .arg(&dist)
        .output()
        .unwrap();
    assert!(
        stage.status.success(),
        "{}{}",
        String::from_utf8_lossy(&stage.stdout),
        String::from_utf8_lossy(&stage.stderr)
    );
    for name in ["example-1.0.0.tar.gz", "example-1.0.0.tar.gz.sha256"] {
        assert_eq!(std::fs::read_to_string(dist.join(name)).unwrap(), name);
    }
}
