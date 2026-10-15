//! The plugin archive preview changes no installed module state.

use std::collections::BTreeMap;

use crate::common::TestDaemon;
use memcastle::domain::sha256_hex;
use memcastle::domain::{
    PluginDependency, PluginInfo, PluginManifest, PluginModule, PluginModuleKind,
};
use memcastle::plugin::package;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn an_unreadable_selected_plugin_catalogue_fails_but_other_configured_catalogues_remain_searchable()
 {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing").display().to_string();
    let catalogue = format!("{}/docs", env!("CARGO_MANIFEST_DIR"));
    let selected = missing.clone();
    let daemon = TestDaemon::start_configured(move |config| {
        config.plugins.registries = vec![selected, catalogue];
    })
    .await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/plugin-registry/search", daemon.base_url))
        .query(&[("registry", &missing)])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let error: Value = response.json().await.unwrap();
    assert_eq!(error["code"], "memcastle::plugin::registry_unavailable");

    let response = client
        .get(format!("{}/api/plugin-registry/search", daemon.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let search: Value = response.json().await.unwrap();
    assert!(!search["plugins"].as_array().unwrap().is_empty());
    assert_eq!(search["warnings"].as_array().unwrap().len(), 1);
    daemon.shutdown().await;
}

#[tokio::test]
async fn previewing_an_empty_plugin_does_not_install_it_or_change_sources() {
    let daemon = TestDaemon::start().await;
    let client = reqwest::Client::new();
    let manifest = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "example".to_string(),
            version: "0.1.0".to_string(),
            provider: "example".to_string(),
            description: "No modules yet".to_string(),
            repository: "https://github.com/example/example".to_string(),
            license: "MIT".to_string(),
        },
        memcastle: ">=0.4".to_string(),
        dependencies: Vec::new(),
        shared_config_schema: None,
        authentication: None,
        modules: Vec::new(),
    };
    let files = BTreeMap::from([(
        "plugin.toml".to_string(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    )]);
    let archive = package::pack(&files).unwrap();
    let response = client
        .post(format!("{}/api/plugins/preview", daemon.base_url))
        .body(archive.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let preview: serde_json::Value = response.json().await.unwrap();
    assert_eq!(preview["id"], "example");
    assert_eq!(preview["modules"].as_array().unwrap().len(), 0);

    let plugins: serde_json::Value = client
        .get(format!("{}/api/plugins", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(plugins.as_array().unwrap().is_empty());

    let sources_before: serde_json::Value = client
        .get(format!("{}/api/sources", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let installed = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(archive)
        .send()
        .await
        .unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let record: serde_json::Value = installed.json().await.unwrap();
    assert_eq!(record["id"], "example");
    let sources_after: serde_json::Value = client
        .get(format!("{}/api/sources", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        sources_before, sources_after,
        "installing a plugin cannot activate a source"
    );
    let mut rewritten = manifest;
    rewritten.plugin.description = "changed without a version bump".into();
    let changed = package::pack(&BTreeMap::from([(
        "plugin.toml".to_string(),
        toml::to_string(&rewritten).unwrap().into_bytes(),
    )]))
    .unwrap();
    let refused = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(changed)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_GATEWAY);
    let error: Value = refused.json().await.unwrap();
    assert_eq!(error["code"], "memcastle::plugin::integrity");
    let removed = client
        .delete(format!("{}/api/plugins/example", daemon.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    let plugins: serde_json::Value = client
        .get(format!("{}/api/plugins", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(plugins.as_array().unwrap().is_empty());
    daemon.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_preexisting_plugin_root_symlink_cannot_redirect_a_generation_outside_the_palace() {
    let root = tempfile::tempdir().unwrap();
    let plugins = root.path().join("plugins");
    let unrelated = root.path().join("unrelated");
    std::fs::create_dir(&unrelated).unwrap();
    let daemon =
        TestDaemon::start_configured(|config| config.plugins.dir = Some(plugins.clone())).await;
    std::os::unix::fs::symlink(&unrelated, &plugins).unwrap();
    let files = BTreeMap::from([(
        "plugin.toml".to_string(),
        b"format = 1\nmemcastle = '>=0.4'\n[plugin]\nid = 'example'\nversion = '0.1.0'\nprovider = 'example'\ndescription = 'example'\nrepository = 'https://github.com/example/example'\nlicense = 'MIT'\n".to_vec(),
    )]);
    let rejected = reqwest::Client::new()
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(package::pack(&files).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert!(std::fs::read_dir(&unrelated).unwrap().next().is_none());
    daemon.shutdown().await;
}

#[tokio::test]
async fn a_local_plugin_catalogue_discovers_and_installs_a_verified_package() {
    let catalogue = tempfile::tempdir().unwrap();
    let manifest = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "example".into(),
            version: "0.1.0".into(),
            provider: "example".into(),
            description: "An empty provider".into(),
            repository: "https://github.com/example/example".into(),
            license: "MIT".into(),
        },
        memcastle: ">=0.4".into(),
        dependencies: Vec::new(),
        shared_config_schema: None,
        authentication: None,
        modules: Vec::new(),
    };
    let archive = package::pack(&BTreeMap::from([(
        "plugin.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    )]))
    .unwrap();
    std::fs::write(catalogue.path().join("example-0.1.0.tar.gz"), &archive).unwrap();
    std::fs::write(catalogue.path().join("plugins.json"), serde_json::json!({
        "format": 1, "name": "offline", "plugins": [{
            "id": "example", "description": "An empty provider", "repository": "example/example", "modules": [],
            "versions": [{"version": "0.1.0", "url": "example-0.1.0.tar.gz", "sha256": sha256_hex(&archive)}],
        }],
    }).to_string()).unwrap();
    let daemon = TestDaemon::start_configured(|config| {
        config.plugins.registries = vec![catalogue.path().display().to_string()];
    })
    .await;
    let client = reqwest::Client::new();
    let missing = client
        .get(format!(
            "{}/api/plugin-registry/plugins/unknown",
            daemon.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let body: Value = missing.json().await.unwrap();
    assert_eq!(body["code"], "memcastle::plugin::not_in_registry");
    let search: serde_json::Value = client
        .get(format!(
            "{}/api/plugin-registry/search?q=example",
            daemon.base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["plugins"][0]["id"], "example");
    let preview: serde_json::Value = client
        .get(format!(
            "{}/api/plugin-registry/plugins/example",
            daemon.base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["archive_digest"], sha256_hex(&archive));
    std::fs::write(
        catalogue.path().join("example-0.1.0.tar.gz"),
        b"replaced after preview",
    )
    .unwrap();
    let rejected = client.post(format!("{}/api/plugin-registry/install", daemon.base_url))
        .json(&serde_json::json!({"id": "example", "version": "0.1.0", "archive_digest": preview["archive_digest"]}))
        .send().await.unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_GATEWAY);
    let reason: serde_json::Value = rejected.json().await.unwrap();
    assert_eq!(reason["code"], "memcastle::plugin::integrity");
    std::fs::write(catalogue.path().join("example-0.1.0.tar.gz"), &archive).unwrap();
    let installed = client.post(format!("{}/api/plugin-registry/install", daemon.base_url))
        .json(&serde_json::json!({"id": "example", "version": "0.1.0", "archive_digest": preview["archive_digest"]}))
        .send().await.unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let installed: serde_json::Value = installed.json().await.unwrap();
    assert_eq!(installed["id"], "example");
    assert_eq!(
        installed["upstream"],
        catalogue.path().display().to_string()
    );
    daemon.shutdown().await;
}

#[tokio::test]
async fn plugin_installation_exposes_an_integration_in_the_local_catalog() {
    let daemon = TestDaemon::start().await;
    let manifest_text = b"format = 1\n[integration]\nid = 'assistant'\nversion = '0.1.0'\ndescription = 'example integration'\n[compatibility]\nmemcastle = '>=0.4'\n[agent]\nkind = 'opencode'\nentry = 'index.js'\n[[assets]]\nfrom = 'dist'\nto = '.'\n";
    let entry = b"export default { id: 'assistant', server: async () => ({}) };\n";
    let manifest = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "example".into(),
            version: "0.1.0".into(),
            provider: "example".into(),
            description: "A plugin with an integration".into(),
            repository: "https://github.com/example/example".into(),
            license: "MIT".into(),
        },
        memcastle: ">=0.4".into(),
        dependencies: Vec::new(),
        shared_config_schema: None,
        authentication: None,
        modules: vec![PluginModule {
            id: "assistant".into(),
            kind: PluginModuleKind::Integration,
            version: "0.1.0".into(),
            manifest: "modules/assistant/memcastle-integration.toml".into(),
            manifest_sha256: sha256_hex(manifest_text),
            entry: "modules/assistant/dist/index.js".into(),
            sha256: sha256_hex(entry),
            optional: false,
            contract: None,
            config_schema: None,
        }],
    };
    let archive = package::pack(&BTreeMap::from([
        (
            "plugin.toml".to_string(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        ),
        (
            "modules/assistant/memcastle-integration.toml".to_string(),
            manifest_text.to_vec(),
        ),
        (
            "modules/assistant/dist/index.js".to_string(),
            entry.to_vec(),
        ),
    ]))
    .unwrap();
    let client = reqwest::Client::new();
    let installed = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(archive.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let plugins_dir = daemon.palace_path.parent().unwrap().join("plugins");
    let catalogues = memcastle::integration::Catalog::plugin_catalogs(&plugins_dir).unwrap();
    assert_eq!(catalogues.len(), 1);
    assert_eq!(catalogues[0].integrations()[0].id(), "assistant");
    let generation = std::fs::read_to_string(plugins_dir.join("example/current")).unwrap();
    let projected = plugins_dir
        .join("example")
        .join(generation)
        .join("integrations/assistant/dist/index.js");
    std::fs::write(&projected, "unreviewed integration code").unwrap();
    let refused = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(archive)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let reason: Value = refused.json().await.unwrap();
    assert_eq!(reason["code"], "memcastle::plugin::package_invalid");
    std::fs::write(&projected, entry).unwrap();
    assert_eq!(
        memcastle::integration::Catalog::plugin_catalogs(&plugins_dir)
            .unwrap()
            .len(),
        1
    );
    let unreviewed = projected.parent().unwrap().join("helper.js");
    std::fs::write(&unreviewed, "export const injected = true").unwrap();
    assert!(
        memcastle::integration::Catalog::plugin_catalogs(&plugins_dir).is_err(),
        "a verified entry must not import a local file absent from the archive"
    );
    std::fs::remove_file(&unreviewed).unwrap();
    #[cfg(unix)]
    {
        let elsewhere = daemon
            .palace_path
            .parent()
            .unwrap()
            .join("identical-integration.js");
        std::fs::write(&elsewhere, entry).unwrap();
        std::fs::remove_file(&projected).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &projected).unwrap();
        assert!(
            memcastle::integration::Catalog::plugin_catalogs(&plugins_dir).is_err(),
            "matching bytes through a link are not a verified plugin asset"
        );
        std::fs::remove_file(&projected).unwrap();
    }
    daemon.shutdown().await;
}

#[tokio::test]
async fn a_direct_github_repository_resolves_its_plugin_id_from_a_verified_release_not_the_repo_name()
 {
    let manifest = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "example".into(),
            version: "0.1.0".into(),
            provider: "example".into(),
            description: "Direct plugin".into(),
            repository: "https://github.com/example/provider-repo".into(),
            license: "MIT".into(),
        },
        memcastle: ">=0.4".into(),
        dependencies: Vec::new(),
        shared_config_schema: None,
        authentication: None,
        modules: Vec::new(),
    };
    let archive = package::pack(&BTreeMap::from([(
        "plugin.toml".to_string(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    )]))
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = format!("http://{}", listener.local_addr().unwrap());
    let listing = serde_json::json!([{ "tag_name": "v0.1.0", "draft": false, "prerelease": false,
        "assets": [{ "name": "example-0.1.0.tar.gz", "state": "uploaded", "size": archive.len(),
            "digest": format!("sha256:{}", sha256_hex(&archive)),
            "browser_download_url": format!("{api}/download/example-0.1.0.tar.gz") }] }]);
    let app = axum::Router::new()
        .route(
            "/repos/example/provider-repo/releases",
            axum::routing::get(move || async move { axum::Json(listing) }),
        )
        .route(
            "/download/example-0.1.0.tar.gz",
            axum::routing::get(move || async move { archive }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let api_for_first = api.clone();
    let daemon = TestDaemon::start_configured(move |config| {
        config.mining.github_api_url = api_for_first;
    })
    .await;
    let repository = "https://github.com/example/provider-repo";
    let client = reqwest::Client::new();
    let search: Value = client
        .get(format!("{}/api/plugin-registry/search", daemon.base_url))
        .query(&[("registry", repository)])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["plugins"][0]["id"], "example", "{search}");
    let preview: Value = client
        .get(format!(
            "{}/api/plugin-registry/plugins/example",
            daemon.base_url
        ))
        .query(&[("registry", repository)])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["plugin"]["id"], "example", "{preview}");
    let installed = client
        .post(format!("{}/api/plugin-registry/install", daemon.base_url))
        .json(
            &json!({"id": "example", "version": "0.1.0", "registry": repository,
            "archive_digest": preview["archive_digest"]}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(
        installed.status(),
        StatusCode::OK,
        "{}",
        installed.text().await.unwrap()
    );
    let cli = tokio::process::Command::new(assert_cmd::cargo::cargo_bin("memcastle"))
        .env("MEMCASTLE_PALACE_PATH", &daemon.palace_path)
        .env_remove("MEMCASTLE_AUTH_ENABLED")
        .env_remove("MEMCASTLE_AUTH_TOKEN")
        .args(["--json", "plugin", "install", repository])
        .output()
        .await
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let via_cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(via_cli["id"], "example");
    daemon.shutdown().await;
    let key = memcastle::source::signing::generate().unwrap();
    let required = TestDaemon::start_configured(move |config| {
        config.mining.github_api_url = api;
        config.plugins.trust = memcastle::config::TrustMode::Required;
        config.plugins.trusted_keys = vec![memcastle::source::signing::public_key_text(
            &key.verifying_key(),
        )];
    })
    .await;
    let refused = client
        .get(format!(
            "{}/api/plugin-registry/plugins/example",
            required.base_url
        ))
        .query(&[("registry", repository)])
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let reason: Value = refused.json().await.unwrap();
    assert_eq!(reason["code"], "memcastle::plugin::untrusted", "{reason}");
    required.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn only_a_required_dependency_prevents_removing_its_provider() {
    let daemon = TestDaemon::start().await;
    let client = reqwest::Client::new();
    for (id, dependencies) in [
        ("base", Vec::new()),
        (
            "dependent",
            vec![PluginDependency {
                id: "base".into(),
                version: ">=0.1.0".into(),
                optional: false,
            }],
        ),
    ] {
        let manifest = PluginManifest {
            format: 1,
            plugin: PluginInfo {
                id: id.into(),
                version: "0.1.0".into(),
                provider: id.into(),
                description: "An empty provider".into(),
                repository: format!("https://github.com/example/{id}"),
                license: "MIT".into(),
            },
            memcastle: ">=0.4".into(),
            dependencies,
            shared_config_schema: None,
            authentication: None,
            modules: Vec::new(),
        };
        let bytes = package::pack(&BTreeMap::from([(
            "plugin.toml".into(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        )]))
        .unwrap();
        let response = client
            .post(format!("{}/api/plugins", daemon.base_url))
            .body(bytes)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let removed = client
        .delete(format!("{}/api/plugins/base", daemon.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::CONFLICT);
    let reason: Value = removed.json().await.unwrap();
    assert_eq!(reason["code"], "memcastle::plugin::blocked");
    let removed = client
        .delete(format!("{}/api/plugins/dependent", daemon.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    let optional = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "optional-dependent".into(),
            version: "0.1.0".into(),
            provider: "optional-dependent".into(),
            description: "Optional dependency".into(),
            repository: "https://github.com/example/optional-dependent".into(),
            license: "MIT".into(),
        },
        memcastle: ">=0.4".into(),
        dependencies: vec![PluginDependency {
            id: "base".into(),
            version: ">=0.1.0".into(),
            optional: true,
        }],
        shared_config_schema: None,
        authentication: None,
        modules: Vec::new(),
    };
    let bytes = package::pack(&BTreeMap::from([(
        "plugin.toml".into(),
        toml::to_string(&optional).unwrap().into_bytes(),
    )]))
    .unwrap();
    let installed = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let removed = client
        .delete(format!("{}/api/plugins/base", daemon.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    assert_eq!(
        client
            .get(format!(
                "{}/api/plugins/optional-dependent",
                daemon.base_url
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    daemon.shutdown().await;
}

#[tokio::test]
async fn restart_rebuilds_the_local_module_pointer_from_the_durable_plugin_record() {
    let root = tempfile::tempdir().unwrap();
    let mut config = memcastle::config::Config::default();
    config.palace.path = root.path().join("palace");
    config.plugins.dir = Some(root.path().join("plugins"));
    config.plugins.registries.clear();
    config.mining.registries.clear();
    config.mining.bundled_dir = Some(root.path().join("no-bundle"));
    config.server.bind = "127.0.0.1".parse().unwrap();
    config.server.port = 0;
    // This in-process restart tests pointer recovery, not fsync durability; avoid the costly flush on CI runners.
    config.store = memcastle::config::StoreConfig::Embedded {
        sync: memcastle::store::StoreSync::Never,
    };
    let client = reqwest::Client::new();
    let archive = package::pack(&BTreeMap::from([(
        "plugin.toml".to_string(),
        toml::to_string(&PluginManifest {
            format: 1,
            plugin: PluginInfo {
                id: "example".into(),
                version: "0.1.0".into(),
                provider: "example".into(),
                description: "Restart fixture".into(),
                repository: "https://github.com/example/example".into(),
                license: "MIT".into(),
            },
            memcastle: ">=0.4".into(),
            dependencies: Vec::new(),
            shared_config_schema: None,
            authentication: None,
            modules: Vec::new(),
        })
        .unwrap()
        .into_bytes(),
    )]))
    .unwrap();
    let handle = tokio::spawn(memcastle::server::run(config.clone()));
    let info = crate::common::wait_for_registry(&config.palace.path).await;
    let url = format!("http://{}", info.bind_addr);
    let installed = client
        .post(format!("{url}/api/plugins"))
        .body(archive)
        .send()
        .await
        .unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let pointer = config.plugins.dir().join("example/current");
    assert!(pointer.is_file());
    client
        .post(format!("{url}/api/shutdown"))
        .send()
        .await
        .unwrap();
    handle.await.unwrap().unwrap();

    std::fs::remove_file(&pointer).unwrap();
    let handle = tokio::spawn(memcastle::server::run(config.clone()));
    let info = crate::common::wait_for_registry(&config.palace.path).await;
    let url = format!("http://{}", info.bind_addr);
    assert!(
        pointer.is_file(),
        "restart must finish an interrupted module-pointer publication"
    );
    let record: Value = client
        .get(format!("{url}/api/plugins/example"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(record["manifest"]["plugin"]["version"], "0.1.0");
    // Simulate a crash after the ledger deletion but before its generation directory was removed.
    let installed_dir = config.plugins.dir().join("example");
    let preserved = root.path().join("preserved-generation");
    std::fs::rename(&installed_dir, &preserved).unwrap();
    let removed = client
        .delete(format!("{url}/api/plugins/example"))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    std::fs::rename(&preserved, &installed_dir).unwrap();
    assert!(pointer.exists());
    client
        .post(format!("{url}/api/shutdown"))
        .send()
        .await
        .unwrap();
    handle.await.unwrap().unwrap();
    let handle = tokio::spawn(memcastle::server::run(config.clone()));
    let info = crate::common::wait_for_registry(&config.palace.path).await;
    assert!(
        !pointer.exists(),
        "restart must retire a pointer without a plugin ledger row"
    );
    let url = format!("http://{}", info.bind_addr);
    let missing = client
        .get(format!("{url}/api/plugins/example"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    client
        .post(format!("{url}/api/shutdown"))
        .send()
        .await
        .unwrap();
    handle.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_plugin_with_a_component_that_cannot_load_leaves_no_module_or_package_installed() {
    let daemon = TestDaemon::start().await;
    let source = b"format = 1\n[source]\nname = 'broken'\nversion = '0.1.0'\ndescription = 'broken component'\n[compatibility]\ncontract = '0.4.0'\nmemcastle = '>=0.4'\n";
    let invalid_component = b"\0asmnot-a-component";
    let manifest = PluginManifest {
        format: 1,
        plugin: PluginInfo {
            id: "example".into(),
            version: "0.1.0".into(),
            provider: "example".into(),
            description: "Invalid component".into(),
            repository: "https://github.com/example/example".into(),
            license: "MIT".into(),
        },
        memcastle: ">=0.4".into(),
        dependencies: Vec::new(),
        shared_config_schema: None,
        authentication: None,
        modules: vec![PluginModule {
            id: "broken".into(),
            kind: PluginModuleKind::Source,
            version: "0.1.0".into(),
            manifest: "modules/broken/memcastle-source.toml".into(),
            manifest_sha256: sha256_hex(source),
            entry: "modules/broken/source.wasm".into(),
            sha256: sha256_hex(invalid_component),
            optional: false,
            contract: Some("0.4.0".into()),
            config_schema: None,
        }],
    };
    let archive = package::pack(&BTreeMap::from([
        (
            "plugin.toml".to_string(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        ),
        (
            "modules/broken/memcastle-source.toml".to_string(),
            source.to_vec(),
        ),
        (
            "modules/broken/source.wasm".to_string(),
            invalid_component.to_vec(),
        ),
    ]))
    .unwrap();
    let client = reqwest::Client::new();
    let rejected = client
        .post(format!("{}/api/plugins", daemon.base_url))
        .body(archive)
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let error: Value = rejected.json().await.unwrap();
    assert_eq!(error["code"], "memcastle::plugin::package_invalid");
    let installed: Value = client
        .get(format!("{}/api/plugins", daemon.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(installed.as_array().unwrap().is_empty());
    daemon.shutdown().await;
}
