//! The local bundle script must not carry source assets after their providers move out of Core.
//!
//! Local packaging must not leak a formerly bundled package from an old output directory.

#![cfg(unix)]

#[test]
fn the_local_package_script_removes_stale_bundled_sources_and_archives() {
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
        "an old source archive must not survive local packaging"
    );
}
