//! The same conformance cases as the built-in adapter, run against a WebAssembly component
//! (docs/adr/026, docs/writing-sources.md).
//!
//! The native counterpart runs in `tests/in_process/native_conformance.rs` on every OS.
//!
//! This binary builds a component, so it is part of the `wasm_` suite.

use std::path::PathBuf;

mod common;

use common::wasm::reference_component;

use memcastle::config::MiningConfig;
use memcastle::mining::wasm::WasmAdapter;
use memcastle::source::build::Project;
use memcastle::source::conformance;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cases() -> PathBuf {
    root().join("tests/fixtures/sources/conformance")
}

fn wasm_directory() -> WasmAdapter {
    let project = Project::open(&root().join("sources/directory")).unwrap();
    // The reference source as `common::wasm` builds it: a debug build in the shared target directory, not the release
    // one `mise run sources:check` makes.
    let (bytes, _) = reference_component();
    WasmAdapter::load(&project.manifest, &bytes, &MiningConfig::default()).unwrap()
}

fn assert_passes(report: &conformance::Report) {
    for case in &report.cases {
        assert!(
            case.failures.is_empty(),
            "case `{}` failed:\n{}",
            case.name,
            case.failures.join("\n")
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_reference_webassembly_source_passes_the_same_conformance_cases() {
    let report = conformance::run_all(&wasm_directory(), &cases())
        .await
        .unwrap();
    assert!(!report.cases.is_empty());
    assert_passes(&report);
}
