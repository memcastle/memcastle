//! The opt-in extension and credential fixtures use the same binary as legacy tests.
mod common;
use memcastle::config::MiningConfig;
use memcastle::domain::{KnownDocument, Options, Permissions, Secret};
use memcastle::mining::adapter::{Acquisition, Reconciliation, SourceAdapter};
use memcastle::mining::wasm::WasmAdapter;
use serde_json::json;

fn manifest(text: &str) -> memcastle::domain::SourceManifest {
    let mut manifest = memcastle::source::manifest::parse(text, &[]).unwrap();
    manifest.compatibility.reconciliation = Some("0.1.0".into());
    manifest.permissions = Permissions {
        credential: true,
        ..Default::default()
    };
    manifest.capabilities.needs_credentials = true;
    manifest
}

#[tokio::test]
async fn the_same_component_supports_legacy_and_explicit_reconciliation_calls() {
    let (component, text) = common::wasm::mock_component();
    let legacy = WasmAdapter::load(
        &memcastle::source::manifest::parse(&text, &[]).unwrap(),
        &component,
        &MiningConfig::default(),
    )
    .unwrap();
    assert!(!legacy.reconciles());
    let legacy_source = legacy
        .identify(Some("reconciled:workspace"), &Options::new())
        .unwrap();
    assert_eq!(
        legacy
            .discover(&legacy_source, &json!(null), 10)
            .await
            .unwrap()
            .candidates
            .len(),
        2
    );
    let adapter = WasmAdapter::load_with_credential(
        &manifest(&text),
        &component,
        &MiningConfig::default(),
        None,
        None,
        Some(Secret::new("memcastle-source-test-credential")),
        None,
    )
    .unwrap();
    assert!(adapter.reconciles());
    let mut source = adapter
        .identify(
            Some("reconciled:workspace"),
            &Options::from([("authenticated".into(), "true".into())]),
        )
        .unwrap();
    let first = adapter.discover(&source, &json!(null), 10).await.unwrap();
    assert!(first.candidates.is_empty());
    assert!(!first.exhausted);
    let page = adapter
        .discover(&source, &first.cursor_after.unwrap(), 10)
        .await
        .unwrap();
    let candidate = &page.candidates[0];
    let Acquisition::Document(raw) = adapter.acquire(&source, candidate).await.unwrap() else {
        panic!("expected a live document");
    };
    assert_eq!(raw.external_id, candidate.external_id);
    assert!(!raw.body.contains("memcastle-source-test-credential"));
    let known = KnownDocument {
        pending: false,
        retired: false,
        external_id: raw.external_id.clone(),
        revision: raw.revision,
        metadata: raw.metadata,
    };
    source
        .options
        .insert("exclude".into(), known.external_id.clone());
    assert_eq!(
        adapter.reconcile(&source, &known).await.unwrap(),
        Reconciliation::Retire("excluded".into())
    );
    assert!(matches!(
        adapter.acquire(&source, candidate).await.unwrap(),
        Acquisition::Retire(_)
    ));
}

#[tokio::test]
async fn offline_conformance_preserves_opaque_locators_and_supplies_only_a_fixed_fixture_credential()
 {
    let (component, text) = common::wasm::mock_component();
    let mut manifest = manifest(&text);
    manifest.test = Some(memcastle::domain::TestSection {
        fixtures: "fixtures".into(),
        fixture_credential: true,
    });
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("dist")).unwrap();
    std::fs::create_dir_all(dir.path().join("fixtures/opaque")).unwrap();
    std::fs::write(
        dir.path().join("memcastle-source.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.path().join("dist/source.wasm"), component).unwrap();
    let case = json!({"name":"opaque","description":"API source with a selected fixture credential","locator":".","opaque_locator":"reconciled:fixture","options":{"authenticated":"true"},"limit":1,"documents":[
        {"external_id":"gazebos","title":"gazebos","kind":"other","segments":["reconciled:fixture gazebos"]},
        {"external_id":"lighthouses","title":"lighthouses","kind":"other","segments":["reconciled:fixture lighthouses"]}
    ]});
    std::fs::write(
        dir.path().join("fixtures/opaque/case.json"),
        case.to_string(),
    )
    .unwrap();
    let mut project = memcastle::source::build::Project::open(dir.path()).unwrap();
    let report = project.test(&MiningConfig::default()).await.unwrap();
    assert!(report.passed(), "{report:?}");
    project.manifest.test.as_mut().unwrap().fixture_credential = false;
    assert!(
        !project
            .test(&MiningConfig::default())
            .await
            .unwrap()
            .passed(),
        "a production credential must never be used as fallback"
    );
}
