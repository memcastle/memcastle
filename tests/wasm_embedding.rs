//! One synthetic component exercises the separate embedding contract without building vendor implementations.

mod common;

use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

use memcastle::config::{
    EmbeddingConnection, EmbeddingProfileConfig, EmbeddingProvider, EmbeddingsConfig,
};
use memcastle::domain::{
    CredentialRef, PluginInfo, PluginManifest, PluginModule, PluginModuleKind, PluginRecord,
    sha256_hex,
};
use memcastle::embed::{Embedder, Embeddings, wasm::WasmEmbedder};
use memcastle::plugin::embedding::{Manifest, Model};
use memcastle::plugin::package;

fn component() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("tests/fixtures/mock-embedding");
    let target = root.join("target/test-mock-embedding");
    let output = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--target-dir"])
        .arg(&target)
        .current_dir(&fixture)
        .output()
        .expect("cargo can build the synthetic component");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read(target.join("wasm32-wasip2/debug/memcastle_test_mock_embedding.wasm")).unwrap()
}

fn manifest() -> Manifest {
    Manifest {
        id: "mock-embedding".into(),
        version: "0.1.0".into(),
        contract: "0.1.0".into(),
        models: vec![Model {
            id: "synthetic".into(),
            dimensions: vec![3],
        }],
        network: false,
        credential: true,
        config_schema: None,
    }
}

#[tokio::test]
async fn a_component_embeds_ordered_batches_without_becoming_a_mining_source() {
    let provider = WasmEmbedder::load(
        manifest(),
        &component(),
        "synthetic".into(),
        3,
        "{}".into(),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(
        provider
            .embed(&["a".into(), "longer".into()])
            .await
            .unwrap(),
        vec![vec![1.0, 0.0, 1.0], vec![6.0, 0.0, 1.0]]
    );
}

#[tokio::test]
async fn a_component_receives_only_the_explicitly_selected_credential() {
    let bytes = component();
    let no_token = WasmEmbedder::load(
        manifest(),
        &bytes,
        "synthetic".into(),
        3,
        "{\"require_token\":true}".into(),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(no_token.embed(&["text".into()]).await.is_err());
    let token = WasmEmbedder::load(
        manifest(),
        &bytes,
        "synthetic".into(),
        3,
        "{\"require_token\":true}".into(),
        Some(CredentialRef::Env {
            name: "PATH".into(),
        }),
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(token.embed(&["text".into()]).await.unwrap().len(), 1);
}

#[test]
fn a_component_that_lies_about_its_capabilities_cannot_load() {
    let mut claimed = manifest();
    claimed.models[0].dimensions = vec![768];
    assert!(
        WasmEmbedder::load(
            claimed,
            &component(),
            "synthetic".into(),
            768,
            "{}".into(),
            None,
            Duration::from_secs(5)
        )
        .is_err()
    );
}

#[tokio::test]
async fn component_time_and_memory_limits_are_enforced_even_when_the_guest_does_not_cooperate() {
    let bytes = component();
    for (settings, timeout) in [
        ("spin", Duration::from_millis(100)),
        ("memory", Duration::from_secs(5)),
    ] {
        let provider = WasmEmbedder::load(
            manifest(),
            &bytes,
            "synthetic".into(),
            3,
            settings.into(),
            None,
            timeout,
        )
        .unwrap();
        let call =
            tokio::time::timeout(Duration::from_secs(10), provider.embed(&["text".into()])).await;
        assert!(
            call.unwrap().is_err(),
            "{settings} must not outlive its sandbox limit"
        );
    }
}

#[tokio::test]
async fn network_is_unavailable_without_the_manifest_grant() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut descriptor = manifest();
    descriptor.network = false;
    let bytes = component();
    let denied = WasmEmbedder::load(
        descriptor.clone(),
        &bytes,
        "synthetic".into(),
        3,
        format!("tcp:{port}"),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(denied.embed(&["text".into()]).await.is_err());
    descriptor.network = true;
    let allowed = WasmEmbedder::load(
        descriptor,
        &bytes,
        "synthetic".into(),
        3,
        format!("tcp:{port}"),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(allowed.embed(&["text".into()]).await.unwrap().len(), 1);
}

#[tokio::test]
async fn an_installed_module_becomes_selectable_without_replacing_other_profiles() {
    let bytes = component();
    let descriptor = b"id = 'mock-embedding'\nversion = '0.1.0'\ncontract = '0.1.0'\ncredential = true\n[[models]]\nid = 'synthetic'\ndimensions = [3]\n";
    let mut config = EmbeddingsConfig::default();
    config.providers.insert(
        "mock".into(),
        EmbeddingConnection {
            provider: EmbeddingProvider::Plugin,
            plugin: Some("demo".into()),
            module: Some("mock-embedding".into()),
            ..Default::default()
        },
    );
    config.profiles.insert(
        "installed".into(),
        EmbeddingProfileConfig {
            provider: "mock".into(),
            model: "synthetic".into(),
            dimensions: 3,
        },
    );
    config.providers.insert(
        "caller".into(),
        EmbeddingConnection {
            provider: EmbeddingProvider::External,
            ..Default::default()
        },
    );
    config.profiles.insert(
        "caller".into(),
        EmbeddingProfileConfig {
            provider: "caller".into(),
            model: "caller-model".into(),
            dimensions: 3,
        },
    );
    config.default = Some("installed".into());
    let embeddings = Embeddings::from_config(&config).unwrap();
    let other_space = embeddings
        .select(Some("caller"))
        .unwrap()
        .space()
        .unwrap()
        .clone();
    assert!(
        embeddings
            .select(Some("installed"))
            .unwrap_err()
            .to_string()
            .contains("install")
    );
    let root = tempfile::tempdir().unwrap();
    let base = root.path().join("demo/digest/embeddings/mock-embedding");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("memcastle-embedding.toml"), descriptor).unwrap();
    std::fs::write(base.join("embedding.wasm"), &bytes).unwrap();
    let now = chrono::Utc::now();
    let record = PluginRecord {
        id: "demo".into(),
        digest: "digest".into(),
        generation: "digest".into(),
        upstream: None,
        signed_by: None,
        installed_at: now,
        updated_at: now,
        manifest: PluginManifest {
            format: 1,
            plugin: PluginInfo {
                id: "demo".into(),
                version: "0.1.0".into(),
                provider: "demo".into(),
                description: "Test".into(),
                repository: "https://example.com/demo".into(),
                license: "MIT".into(),
            },
            memcastle: ">=0.4".into(),
            dependencies: vec![],
            shared_config_schema: None,
            authentication: None,
            modules: vec![PluginModule {
                id: "mock-embedding".into(),
                kind: PluginModuleKind::EmbeddingProvider,
                version: "0.1.0".into(),
                manifest: "embeddings/mock-embedding/memcastle-embedding.toml".into(),
                manifest_sha256: sha256_hex(descriptor),
                entry: "embeddings/mock-embedding/embedding.wasm".into(),
                sha256: sha256_hex(&bytes),
                optional: false,
                contract: Some("0.1.0".into()),
                config_schema: None,
            }],
            artifacts: vec![],
            checksums: Default::default(),
        },
    };
    embeddings
        .load_plugins(&config, std::slice::from_ref(&record), root.path())
        .unwrap();
    let selected = embeddings.select(None).unwrap();
    assert_eq!(
        selected.embed_one("test").await.unwrap(),
        vec![4.0, 0.0, 1.0]
    );
    assert_eq!(
        selected.space().unwrap().provider_type,
        "plugin:demo/mock-embedding"
    );
    assert_eq!(
        embeddings.select(Some("caller")).unwrap().space(),
        Some(&other_space)
    );
    let changed_descriptor = b"id = 'mock-embedding'\nversion = '0.1.0'\ncontract = '0.1.0'\ncredential = true\nnetwork = true\n[[models]]\nid = 'synthetic'\ndimensions = [3]\n";
    std::fs::write(base.join("memcastle-embedding.toml"), changed_descriptor).unwrap();
    let mut replacement = record;
    replacement.manifest.modules[0].manifest_sha256 = sha256_hex(changed_descriptor);
    embeddings
        .load_plugins(&config, &[replacement], root.path())
        .unwrap();
    assert_ne!(
        embeddings.select(None).unwrap().space(),
        selected.space(),
        "a changed module manifest creates a new space"
    );
    embeddings.load_plugins(&config, &[], root.path()).unwrap();
    assert!(embeddings.select(Some("installed")).is_err());
    assert_eq!(
        embeddings.select(Some("caller")).unwrap().space(),
        Some(&other_space)
    );
}

fn release(descriptor: &[u8]) -> Vec<u8> {
    let bytes = component();
    let parent = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "example".into(),
            version: "0.1.0".into(),
            provider: "example".into(),
            description: "Embedding provider".into(),
            repository: "https://github.com/example/example".into(),
            license: "MIT".into(),
        },
        memcastle: ">=0.4".into(),
        dependencies: vec![],
        shared_config_schema: None,
        authentication: None,
        modules: vec![PluginModule {
            id: "mock-embedding".into(),
            kind: PluginModuleKind::EmbeddingProvider,
            version: "0.1.0".into(),
            manifest: "embeddings/mock-embedding/memcastle-embedding.toml".into(),
            manifest_sha256: String::new(),
            entry: "embeddings/mock-embedding/embedding.wasm".into(),
            sha256: String::new(),
            optional: false,
            contract: Some("0.1.0".into()),
            config_schema: None,
        }],
        artifacts: vec![],
        checksums: BTreeMap::new(),
    };
    let files = BTreeMap::from([
        (
            "plugin.toml".into(),
            toml::to_string(&parent).unwrap().into_bytes(),
        ),
        (
            "embeddings/mock-embedding/memcastle-embedding.toml".into(),
            descriptor.to_vec(),
        ),
        ("embeddings/mock-embedding/embedding.wasm".into(), bytes),
    ]);
    package::pack(&files).unwrap()
}

#[tokio::test]
async fn a_plugin_install_activates_only_its_configured_embedding_profile() {
    let archive = release(b"id = 'mock-embedding'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'synthetic'\ndimensions = [3]\n");
    let daemon = common::TestDaemon::start_configured(|config| {
        config.embeddings.providers.insert(
            "synthetic".into(),
            EmbeddingConnection {
                provider: EmbeddingProvider::Plugin,
                plugin: Some("example".into()),
                module: Some("mock-embedding".into()),
                ..Default::default()
            },
        );
        config.embeddings.profiles.insert(
            "synthetic".into(),
            EmbeddingProfileConfig {
                provider: "synthetic".into(),
                model: "synthetic".into(),
                dimensions: 3,
            },
        );
        config.embeddings.default = Some("synthetic".into());
    })
    .await;
    let client = reqwest::Client::new();
    let before: serde_json::Value = client
        .get(format!("{}/api/embedding-providers", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(before.as_array().unwrap().is_empty());
    let endpoint = format!("{}/api/jobs", daemon.base_url);
    let submit = || {
        client
            .post(&endpoint)
            .json(&serde_json::json!({"type": "embed", "profile": "synthetic"}))
    };
    let unavailable = submit().send().await.unwrap();
    assert!(
        !unavailable.status().is_success(),
        "the profile cannot run before installation"
    );
    let error: serde_json::Value = unavailable.json().await.unwrap();
    assert_eq!(error["code"], "memcastle::embed::profile_unavailable");
    let incompatible = release(b"id = 'mock-embedding'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'synthetic'\ndimensions = [768]\n");
    let incompatible = client
        .post(format!("{}/api/plugins/preview", daemon.base_url))
        .body(incompatible)
        .send()
        .await
        .unwrap();
    assert!(
        !incompatible.status().is_success(),
        "a configured profile cannot select unsupported dimensions"
    );
    let preview = client
        .post(format!("{}/api/plugins/preview", daemon.base_url))
        .body(archive.clone())
        .send()
        .await
        .unwrap();
    assert!(
        preview.status().is_success(),
        "{}",
        preview.text().await.unwrap()
    );
    let response = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(archive)
        .send()
        .await
        .unwrap();
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap()
    );
    let discovered: serde_json::Value = client
        .get(format!("{}/api/embedding-providers", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(discovered[0]["module"], "mock-embedding");
    assert_eq!(
        discovered[0]["models"][0]["dimensions"],
        serde_json::json!([3])
    );
    assert_eq!(discovered[0]["profiles"], serde_json::json!(["synthetic"]));
    assert!(
        submit().send().await.unwrap().status().is_success(),
        "install refreshes selection without restart"
    );
    let removed = client
        .delete(format!("{}/api/plugins/example", daemon.base_url))
        .send()
        .await
        .unwrap();
    assert!(
        !removed.status().is_success(),
        "a configured profile protects its module"
    );
    daemon.shutdown().await;
}

#[tokio::test]
async fn an_embedding_network_grant_requires_its_own_reviewed_consent() {
    let archive = release(b"id = 'mock-embedding'\nversion = '0.1.0'\ncontract = '0.1.0'\nnetwork = true\n[[models]]\nid = 'synthetic'\ndimensions = [3]\n");
    let daemon = common::TestDaemon::start().await;
    let client = reqwest::Client::new();
    let preview = client
        .post(format!("{}/api/plugins/preview", daemon.base_url))
        .body(archive.clone())
        .send()
        .await
        .unwrap();
    assert!(
        preview.status().is_success(),
        "{}",
        preview.text().await.unwrap()
    );
    let preview: serde_json::Value = preview.json().await.unwrap();
    let consent = &preview["embedding_consents"][0];
    assert_eq!(consent["source"], "embedding:mock-embedding");
    assert_eq!(consent["already_granted"], false);
    let endpoint = format!("{}/api/plugins", daemon.base_url);
    let refused = client
        .post(&endpoint)
        .body(archive.clone())
        .send()
        .await
        .unwrap();
    assert!(!refused.status().is_success());
    let error: serde_json::Value = refused.json().await.unwrap();
    assert_eq!(
        error["code"],
        "memcastle::plugin::embedding_consent_required"
    );
    let accepted = client
        .post(&endpoint)
        .header(
            "x-memcastle-consents",
            serde_json::json!({"embedding:mock-embedding": consent["digest"]}).to_string(),
        )
        .body(archive)
        .send()
        .await
        .unwrap();
    assert!(
        accepted.status().is_success(),
        "{}",
        accepted.text().await.unwrap()
    );
    daemon.shutdown().await;
}
