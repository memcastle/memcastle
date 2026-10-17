//! The installed plugin ledger; code bytes stay in immutable generation directories.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::domain::{PluginManifest, PluginRecord, SourcePackageRecord};
use crate::error::{Error, Result};

use super::SurrealStore;

const COLUMNS: &str = "name, manifest, digest, generation, upstream, signed_by, <string>installed_at AS installed_at, <string>updated_at AS updated_at";

#[derive(Deserialize)]
struct Row {
    name: String,
    manifest: Value,
    digest: String,
    generation: String,
    upstream: Option<String>,
    #[serde(default)]
    signed_by: Option<String>,
    installed_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Row {
    fn into_record(self) -> Result<PluginRecord> {
        let manifest: PluginManifest = serde_json::from_value(self.manifest)
            .map_err(|e| Error::store_malformed(format!("plugin manifest: {e}")))?;
        Ok(PluginRecord {
            id: self.name,
            manifest,
            digest: self.digest,
            generation: self.generation,
            upstream: self.upstream,
            signed_by: self.signed_by,
            installed_at: self.installed_at,
            updated_at: self.updated_at,
        })
    }
}

impl SurrealStore {
    /// Who has ever owned this source ID through a plugin, including after its code was removed.
    pub async fn get_plugin_source_owner(&self, source: &str) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Owner {
            plugin: String,
        }
        let mut response = self.db.query("SELECT plugin FROM plugin_source_owner WHERE id = type::record('plugin_source_owner', $name)")
            .bind(("name", source.to_string())).await?;
        let mut owners: Vec<Owner> = super::take_rows(&mut response, 0)?;
        Ok(owners.pop().map(|owner| owner.plugin))
    }
    /// Source IDs ever claimed by a plugin, retained after removal to prevent cursor takeover.
    pub async fn list_plugin_source_owners(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Owner {
            source: String,
        }
        let mut response = self
            .db
            .query("SELECT source FROM plugin_source_owner")
            .await?;
        let owners: Vec<Owner> = super::take_rows(&mut response, 0)?;
        Ok(owners.into_iter().map(|owner| owner.source).collect())
    }
    /// Remove the package and only the source-module installation rows in one transaction.
    /// Mined content, source cursors and drawers live in other tables and are not touched.
    pub async fn delete_plugin_with_sources(
        &self,
        id: &str,
        sources: &[String],
        modules: &[String],
    ) -> Result<()> {
        let mut sql = String::from("BEGIN TRANSACTION;");
        for index in 0..sources.len() {
            sql.push_str(&format!(
                " DELETE type::record('source_package', $source{index});"
            ));
        }
        for index in 0..modules.len() {
            sql.push_str(&format!(
                " DELETE type::record('plugin_module_owner', $module{index});"
            ));
        }
        sql.push_str(" DELETE type::record('plugin', $plugin_id); COMMIT TRANSACTION;");
        let mut query = self.db.query(sql).bind(("plugin_id", id.to_string()));
        for (index, source) in sources.iter().enumerate() {
            query = query.bind((format!("source{index}"), source.clone()));
        }
        for (index, module) in modules.iter().enumerate() {
            query = query.bind((format!("module{index}"), module.clone()));
        }
        super::checked(query.await?)?;
        Ok(())
    }
    /// Atomically switch a plugin release and every source module that points into its generation.
    ///
    /// The archive bytes are staged before this call; no reader sees mixed old/new module rows after a commit.
    pub async fn publish_plugin_with_sources(
        &self,
        plugin: &PluginRecord,
        sources: &[SourcePackageRecord],
        retired_sources: &[String],
        retired_modules: &[String],
    ) -> Result<()> {
        let mut sql = String::from(
            "BEGIN TRANSACTION; UPSERT type::record('plugin', $plugin_id) SET name = $plugin_id, manifest = $manifest, digest = $digest, generation = $generation, upstream = $upstream, signed_by = $signed_by, installed_at = <datetime>$installed_at, updated_at = <datetime>$updated_at;",
        );
        for (index, _) in plugin.manifest.modules.iter().enumerate() {
            // All kinds share one live ID namespace. An integration cannot become ambiguous after two installs
            // race past application-level preflight; historical source ownership remains separately permanent.
            sql.push_str(&format!(" LET $module_owner{index} = (SELECT plugin FROM plugin_module_owner WHERE id = type::record('plugin_module_owner', $module{index}_name)); IF array::len($module_owner{index}) > 0 && $module_owner{index}[0].plugin != $plugin_id {{ THROW 'plugin module ownership conflict'; }}; UPSERT type::record('plugin_module_owner', $module{index}_name) SET name = $module{index}_name, plugin = $plugin_id;"));
        }
        for index in 0..sources.len() {
            // Preflight can race another daemon. The ownership guard belongs inside the publication transaction,
            // before any module or plugin row is overwritten, so only one publisher can claim a source ID.
            sql.push_str(&format!(" LET $owner{index} = (SELECT plugin FROM plugin_source_owner WHERE id = type::record('plugin_source_owner', $source{index}_name)); IF array::len($owner{index}) > 0 && $owner{index}[0].plugin != $plugin_id {{ THROW 'plugin source ownership conflict'; }};"));
            sql.push_str(&format!(" LET $existing{index} = (SELECT plugin, state FROM source_package WHERE id = type::record('source_package', $source{index}_name)); IF array::len($existing{index}) > 0 && $existing{index}[0].plugin != $plugin_id {{ THROW 'plugin source package conflict'; }};"));
            // Source enable/disable is a separate administrative operation; a concurrent plugin update must not
            // restore the state it read during preflight over a more recent decision in this same palace.
            sql.push_str(&format!(" LET $state{index} = IF array::len($existing{index}) > 0 {{ $existing{index}[0].state }} ELSE {{ $source{index}_state }};"));
            sql.push_str(&format!(" UPSERT type::record('source_package', $source{index}_name) SET name = $source{index}_name, state = $state{index}, digest = $source{index}_digest, manifest = $source{index}_manifest, installed_at = <datetime>$source{index}_installed, updated_at = <datetime>$source{index}_updated, origin = $source{index}_origin, registry = NONE, archive_digest = $digest, signed_by = $signed_by, plugin = $plugin_id, generation = $generation;"));
            sql.push_str(&format!(" UPSERT type::record('plugin_source_owner', $source{index}_name) SET source = $source{index}_name, plugin = $plugin_id;"));
        }
        for index in 0..retired_sources.len() {
            sql.push_str(&format!(
                " DELETE type::record('source_package', $retired{index});"
            ));
        }
        for index in 0..retired_modules.len() {
            sql.push_str(&format!(
                " DELETE type::record('plugin_module_owner', $retired_module{index});"
            ));
        }
        sql.push_str(" COMMIT TRANSACTION;");
        let mut query = self
            .db
            .query(sql)
            .bind(("plugin_id", plugin.id.clone()))
            .bind(("manifest", super::bindable(&plugin.manifest)?))
            .bind(("digest", plugin.digest.clone()))
            .bind(("generation", plugin.generation.clone()))
            .bind(("upstream", plugin.upstream.clone()))
            .bind(("signed_by", plugin.signed_by.clone()))
            .bind(("installed_at", super::stored(plugin.installed_at)))
            .bind(("updated_at", super::stored(plugin.updated_at)));
        for (index, module) in plugin.manifest.modules.iter().enumerate() {
            query = query.bind((format!("module{index}_name"), module.id.clone()));
        }
        for (index, source) in sources.iter().enumerate() {
            query = query
                .bind((format!("source{index}_name"), source.name.clone()))
                .bind((
                    format!("source{index}_state"),
                    super::bindable(&source.state)?,
                ))
                .bind((format!("source{index}_digest"), source.digest.clone()))
                .bind((
                    format!("source{index}_manifest"),
                    super::bindable(&source.manifest)?,
                ))
                .bind((
                    format!("source{index}_installed"),
                    super::stored(source.installed_at),
                ))
                .bind((
                    format!("source{index}_updated"),
                    super::stored(source.updated_at),
                ))
                .bind((
                    format!("source{index}_origin"),
                    super::bindable(&source.origin)?,
                ));
        }
        for (index, name) in retired_sources.iter().enumerate() {
            query = query.bind((format!("retired{index}"), name.clone()));
        }
        for (index, name) in retired_modules.iter().enumerate() {
            query = query.bind((format!("retired_module{index}"), name.clone()));
        }
        query
            .await
            .map_err(Error::from)
            .and_then(super::checked)
            .map_err(|error| {
                if matches!(&error, Error::Store { source }
                if source.to_string().contains("plugin source ownership conflict")
                    || source.to_string().contains("plugin source package conflict")
                    || source.to_string().contains("plugin module ownership conflict"))
                {
                    Error::PluginBlocked {
                    name: plugin.id.clone(),
                    reason:
                        "a module ID was claimed by another installation; preview the release again"
                            .into(),
                }
                } else {
                    error
                }
            })?;
        Ok(())
    }
    /// Read a plugin by its stable ID.
    pub async fn get_plugin(&self, name: &str) -> Result<Option<PluginRecord>> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {COLUMNS} FROM plugin WHERE id = type::record('plugin', $name)"
            ))
            .bind(("name", name.to_string()))
            .await?;
        let mut rows: Vec<Row> = super::take_rows(&mut response, 0)?;
        rows.pop().map(Row::into_record).transpose()
    }

    /// Read all installed plugin records in stable order.
    pub async fn list_plugins(&self) -> Result<Vec<PluginRecord>> {
        let mut response = self
            .db
            .query(format!("SELECT {COLUMNS} FROM plugin ORDER BY name"))
            .await?;
        let rows: Vec<Row> = super::take_rows(&mut response, 0)?;
        rows.into_iter().map(Row::into_record).collect()
    }

    /// Publish a checked artifact generation to the installation ledger.
    pub async fn save_plugin(&self, record: &PluginRecord) -> Result<()> {
        self.db.query("UPSERT type::record('plugin', $name) SET name = $name, manifest = $manifest, digest = $digest, generation = $generation, upstream = $upstream, signed_by = $signed_by, installed_at = <datetime>$installed_at, updated_at = <datetime>$updated_at")
            .bind(("name", record.id.clone()))
            .bind(("manifest", super::bindable(&record.manifest)?))
            .bind(("digest", record.digest.clone()))
            .bind(("generation", record.generation.clone()))
            .bind(("upstream", record.upstream.clone()))
            .bind(("signed_by", record.signed_by.clone()))
            .bind(("installed_at", super::stored(record.installed_at)))
            .bind(("updated_at", super::stored(record.updated_at)))
            .await?.check()?;
        Ok(())
    }

    /// Remove only the plugin ledger; callers first check module/dependency state.
    pub async fn delete_plugin(&self, id: &str) -> Result<()> {
        self.db
            .query("DELETE type::record('plugin', $name)")
            .bind(("name", id.to_string()))
            .await?
            .check()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        PluginInfo, PluginModule, PluginModuleKind, SourceOrigin, SourcePackageState,
    };
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn replacing_a_plugin_generation_keeps_one_installation_record() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let now = Utc::now();
        let mut record = PluginRecord {
            id: "example".to_string(),
            manifest: PluginManifest {
                format: 1,
                plugin: PluginInfo {
                    id: "example".to_string(),
                    version: "0.1.0".to_string(),
                    provider: "example".to_string(),
                    description: "Demo".to_string(),
                    repository: "https://github.com/example/example".to_string(),
                    license: "MIT".to_string(),
                },
                memcastle: ">=0.4".to_string(),
                dependencies: Vec::new(),
                shared_config_schema: None,
                authentication: None,
                modules: Vec::new(),
                checksums: BTreeMap::new(),
            },
            digest: "old".to_string(),
            generation: "example/old".to_string(),
            upstream: None,
            signed_by: None,
            installed_at: now,
            updated_at: now,
        };
        store.save_plugin(&record).await.unwrap();
        record.generation = "example/new".to_string();
        record.digest = "new".to_string();
        store.save_plugin(&record).await.unwrap();
        let found = store.get_plugin("example").await.unwrap().unwrap();
        assert_eq!(found.generation, "example/new");
        assert_eq!(found.installed_at, now);
        assert_eq!(store.list_plugins().await.unwrap().len(), 1);
        store.delete_plugin("example").await.unwrap();
        assert!(store.list_plugins().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn publishing_two_source_modules_switches_their_ownership_together() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let now = Utc::now();
        let plugin = PluginRecord {
            id: "example".to_string(),
            manifest: PluginManifest {
                format: 1,
                plugin: PluginInfo {
                    id: "example".to_string(),
                    version: "0.1.0".to_string(),
                    provider: "example".to_string(),
                    description: "Two sources".to_string(),
                    repository: "https://github.com/example/example".to_string(),
                    license: "MIT".to_string(),
                },
                memcastle: ">=0.4".to_string(),
                dependencies: Vec::new(),
                shared_config_schema: None,
                authentication: None,
                modules: Vec::new(),
                checksums: BTreeMap::new(),
            },
            digest: "a".repeat(64),
            generation: "a".repeat(64),
            upstream: None,
            signed_by: None,
            installed_at: now,
            updated_at: now,
        };
        let source = |name: &str| {
            SourcePackageRecord {
            name: name.to_string(), state: SourcePackageState::Installed,
            digest: "b".repeat(64),
            manifest: crate::source::manifest::parse(
                &format!("[source]\nname = '{name}'\nversion = '0.1.0'\ndescription = 'one'\n[compatibility]\ncontract = '0.4.0'\nmemcastle = '>=0.4'\n"), &[],
            ).unwrap(),
            installed_at: now, updated_at: now, origin: SourceOrigin::Plugin,
            registry: None, archive_digest: Some(plugin.digest.clone()), signed_by: None,
            plugin: Some(plugin.id.clone()), generation: Some(plugin.generation.clone()),
        }
        };
        store
            .publish_plugin_with_sources(&plugin, &[source("one"), source("two")], &[], &[])
            .await
            .unwrap();
        assert_eq!(store.list_plugins().await.unwrap().len(), 1);
        for name in ["one", "two"] {
            let found = store.get_source_package(name).await.unwrap().unwrap();
            assert_eq!(found.plugin.as_deref(), Some("example"));
            assert_eq!(found.state, SourcePackageState::Installed);
        }
        let mut activated = store.get_source_package("one").await.unwrap().unwrap();
        activated.state = SourcePackageState::Enabled;
        store.save_source_package(&activated).await.unwrap();
        store
            .publish_plugin_with_sources(&plugin, &[source("one")], &["two".to_string()], &[])
            .await
            .unwrap();
        assert!(store.get_source_package("two").await.unwrap().is_none());
        assert_eq!(
            store
                .get_source_package("one")
                .await
                .unwrap()
                .unwrap()
                .state,
            SourcePackageState::Enabled,
            "the user's activation after a plugin preview must survive the generation switch"
        );
        assert!(
            store
                .list_plugin_source_owners()
                .await
                .unwrap()
                .contains(&"two".to_string())
        );
    }

    #[tokio::test]
    async fn another_plugin_cannot_reassign_a_source_or_publish_its_own_record() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let now = Utc::now();
        let manifest = PluginManifest {
            format: 1,
            plugin: PluginInfo {
                id: "first".into(),
                version: "0.1.0".into(),
                provider: "first".into(),
                description: "First owner".into(),
                repository: "https://github.com/example/first".into(),
                license: "MIT".into(),
            },
            memcastle: ">=0.4".into(),
            dependencies: Vec::new(),
            shared_config_schema: None,
            authentication: None,
            modules: Vec::new(),
            checksums: BTreeMap::new(),
        };
        let first = PluginRecord {
            id: "first".into(),
            manifest: manifest.clone(),
            digest: "a".repeat(64),
            generation: "a".repeat(64),
            upstream: None,
            signed_by: None,
            installed_at: now,
            updated_at: now,
        };
        let mut second = first.clone();
        second.id = "second".into();
        second.manifest.plugin.id = second.id.clone();
        let mut source = SourcePackageRecord {
            name: "shared".into(),
            state: SourcePackageState::Installed,
            digest: "b".repeat(64),
            manifest: crate::source::manifest::parse(
                "[source]\nname = 'shared'\nversion = '0.1.0'\ndescription = 'shared'\n[compatibility]\ncontract = '0.4.0'\nmemcastle = '>=0.4'\n",
                &[],
            )
            .unwrap(),
            installed_at: now,
            updated_at: now,
            origin: SourceOrigin::Plugin,
            registry: None,
            archive_digest: Some(first.digest.clone()),
            signed_by: None,
            plugin: Some(first.id.clone()),
            generation: Some(first.generation.clone()),
        };
        store
            .publish_plugin_with_sources(&first, &[source.clone()], &[], &[])
            .await
            .unwrap();
        source.plugin = Some(second.id.clone());
        let error = store
            .publish_plugin_with_sources(&second, &[source], &[], &[])
            .await
            .unwrap_err();
        assert!(matches!(error, Error::PluginBlocked { .. }), "{error}");
        assert!(store.get_plugin("second").await.unwrap().is_none());
        assert_eq!(
            store
                .get_plugin_source_owner("shared")
                .await
                .unwrap()
                .as_deref(),
            Some("first")
        );
        assert_eq!(
            store
                .get_source_package("shared")
                .await
                .unwrap()
                .unwrap()
                .plugin
                .as_deref(),
            Some("first")
        );
        // An installed legacy source has no owner marker, but its package row must still block a plugin takeover.
        let legacy = SourcePackageRecord {
            name: "legacy".into(),
            manifest: crate::source::manifest::parse(
                "[source]\nname = 'legacy'\nversion = '0.1.0'\ndescription = 'legacy'\n[compatibility]\ncontract = '0.4.0'\nmemcastle = '>=0.4'\n",
                &[],
            )
            .unwrap(),
            origin: SourceOrigin::Package,
            plugin: None,
            archive_digest: None,
            generation: None,
            ..store.get_source_package("shared").await.unwrap().unwrap()
        };
        store.save_source_package(&legacy).await.unwrap();
        let mut takeover = legacy.clone();
        takeover.plugin = Some(second.id.clone());
        takeover.origin = SourceOrigin::Plugin;
        assert!(matches!(
            store
                .publish_plugin_with_sources(&second, &[takeover], &[], &[])
                .await,
            Err(Error::PluginBlocked { .. })
        ));
        assert_eq!(
            store
                .get_source_package("legacy")
                .await
                .unwrap()
                .unwrap()
                .origin,
            SourceOrigin::Package
        );
    }

    #[tokio::test]
    async fn two_plugins_cannot_claim_the_same_integration_id_even_when_both_previews_succeeded() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let now = Utc::now();
        let first = PluginRecord {
            id: "first".into(),
            manifest: PluginManifest {
                format: 1,
                plugin: PluginInfo {
                    id: "first".into(),
                    version: "0.1.0".into(),
                    provider: "first".into(),
                    description: "First integration".into(),
                    repository: "https://github.com/example/first".into(),
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
                    manifest_sha256: "a".repeat(64),
                    entry: "modules/assistant/dist/index.js".into(),
                    sha256: "b".repeat(64),
                    optional: false,
                    contract: None,
                    config_schema: None,
                }],
                checksums: BTreeMap::new(),
            },
            digest: "a".repeat(64),
            generation: "a".repeat(64),
            upstream: None,
            signed_by: None,
            installed_at: now,
            updated_at: now,
        };
        let mut second = first.clone();
        second.id = "second".into();
        second.manifest.plugin.id = "second".into();
        store
            .publish_plugin_with_sources(&first, &[], &[], &[])
            .await
            .unwrap();
        let error = store
            .publish_plugin_with_sources(&second, &[], &[], &[])
            .await
            .unwrap_err();
        assert!(matches!(error, Error::PluginBlocked { .. }), "{error:?}");
        assert!(store.get_plugin("second").await.unwrap().is_none());
        store
            .delete_plugin_with_sources("first", &[], &["assistant".into()])
            .await
            .unwrap();
        store
            .publish_plugin_with_sources(&second, &[], &[], &[])
            .await
            .unwrap();
        assert!(store.get_plugin("second").await.unwrap().is_some());
    }
}
