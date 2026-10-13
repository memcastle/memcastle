//! Keep the built-in directory adapter's shared contract in the cross-platform basic suite.

use memcastle::mining::adapters::directory::DirectoryAdapter;
use memcastle::source::conformance;

#[tokio::test]
async fn the_native_directory_source_passes_every_conformance_case() {
    let cases =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sources/conformance");
    let report = conformance::run_all(&DirectoryAdapter::new(2 * 1024 * 1024), &cases)
        .await
        .unwrap();
    assert!(!report.cases.is_empty());
    for case in &report.cases {
        assert!(
            case.failures.is_empty(),
            "case `{}` failed:\n{}",
            case.name,
            case.failures.join("\n")
        );
    }
}
