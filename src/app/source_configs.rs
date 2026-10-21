//! Source-native settings shared by miners, with credential references owned by the source configuration.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::Mutex;

use super::{AppServices, miners::credential_available};
use crate::config::miners_file::{self, FileStamp};
use crate::domain::{CredentialRef, MemoryMode, SourceConfiguration};
use crate::error::{Error, Result};

fn broadening_reasons(
    before: Option<&SourceConfiguration>,
    after: &SourceConfiguration,
    new_options: &crate::domain::Options,
    specs: Option<&[crate::domain::OptionSpec]>,
) -> Result<Vec<String>> {
    if before.is_some_and(|before| before.source != after.source) {
        return Ok(vec!["the source changed".into()]);
    }
    // A known source's unsaved default is already in use by miners; saving it for the first time
    // must be compared with that empty default, not treated as a brand-new independent instance.
    if before.is_none() && (after.id != after.source || specs.is_none()) {
        return Ok(Vec::new());
    }
    let old_options = before.map_or_else(
        || Ok(crate::domain::Options::new()),
        SourceConfiguration::options,
    )
    .map_err(|reason| Error::invalid_input("options", reason))?;
    Ok(crate::domain::option_broadening(
        &old_options,
        new_options,
        specs.unwrap_or_default(),
    ))
}

/// A source configuration without a credential's variable name, path or secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfigurationView {
    /// User-facing configuration ID.
    pub id: String,
    /// Installed source adapter.
    pub source: String,
    /// Source-native defaults (never secret-looking fields).
    pub options: Map<String, Value>,
    /// Credential kind, without its variable name, path or token.
    pub credential_kind: Option<String>,
    /// Whether the credential resolves right now.
    pub credential_available: bool,
}

/// One administrative change; omitted fields are kept.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourceConfigurationPatch {
    /// Installed source; required for a new configuration.
    pub source: Option<String>,
    /// Credential reference, never its contents.
    pub credential: Option<CredentialRef>,
    /// Source-native defaults to set.
    pub options: Map<String, Value>,
    /// Option keys to clear.
    pub unset_options: Vec<String>,
    /// Forget the reference.
    pub unset_credential: bool,
    /// Confirm an intentional change that may select more material for every miner using this instance.
    pub allow_broaden: bool,
}

struct ConfigState {
    configs: Vec<SourceConfiguration>,
    stamp: Option<Option<FileStamp>>,
}

/// The global TOML file is the source of truth; only the daemon rewrites its source-configuration section.
pub struct SourceConfigRegistry {
    path: Option<PathBuf>,
    state: Mutex<ConfigState>,
}

impl SourceConfigRegistry {
    /// Start with the configuration file's startup snapshot.
    #[must_use]
    pub fn new(path: Option<PathBuf>, initial: Vec<SourceConfiguration>) -> Self {
        Self {
            path,
            state: Mutex::new(ConfigState {
                configs: initial,
                stamp: None,
            }),
        }
    }

    fn refresh(&self, state: &mut ConfigState) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let stamp = miners_file::stamp(path)?;
        if state.stamp != Some(stamp) {
            let (configs, read_stamp) = miners_file::read_source_configs(path)?;
            state.configs = configs;
            state.stamp = Some(read_stamp);
        }
        Ok(())
    }

    async fn snapshot(&self) -> Result<Vec<SourceConfiguration>> {
        let mut state = self.state.lock().await;
        self.refresh(&mut state)?;
        Ok(state.configs.clone())
    }
}

impl Default for SourceConfigRegistry {
    fn default() -> Self {
        Self::new(None, Vec::new())
    }
}

fn view(config: &SourceConfiguration) -> SourceConfigurationView {
    let (credential_kind, credential_available) = match &config.credential {
        Some(credential) => (
            Some(
                match credential {
                    CredentialRef::Env { .. } => "env",
                    CredentialRef::File { .. } => "file",
                    CredentialRef::Oauth => "oauth",
                }
                .to_string(),
            ),
            credential_available(credential),
        ),
        None => (None, false),
    };
    SourceConfigurationView {
        id: config.id.clone(),
        source: config.source.clone(),
        options: config.options.clone(),
        credential_kind,
        credential_available,
    }
}

impl AppServices {
    async fn source_config_view(&self, config: &SourceConfiguration) -> SourceConfigurationView {
        let mut answer = view(config);
        if let Some(requirement) = self.oauth_requirement(&config.source).await
            && !matches!(
                config.credential,
                Some(CredentialRef::Env { .. } | CredentialRef::File { .. })
            )
        {
            answer.credential_kind = Some("oauth".into());
            answer.credential_available = self.signed_in(&config.id, requirement).await;
        }
        answer
    }
    pub(super) async fn source_configuration_ids(&self, source: &str) -> Result<Vec<String>> {
        let mut ids = vec![source.to_string()];
        ids.extend(
            self.source_configs
                .snapshot()
                .await?
                .into_iter()
                .filter(|config| config.source == source && config.id != source)
                .map(|config| config.id),
        );
        Ok(ids)
    }
    #[must_use]
    /// Configure the daemon to read these source configurations from its own file.
    pub fn with_source_configs(mut self, configs: SourceConfigRegistry) -> Self {
        self.source_configs = Arc::new(configs);
        self
    }

    /// Include the implicit default configuration for each available source, overridden by an explicit default entry.
    pub async fn list_source_configs(
        &self,
        mode: MemoryMode,
    ) -> Result<Vec<SourceConfigurationView>> {
        Self::require_read(mode, "source_config_list")?;
        let mut configs = self.source_configs.snapshot().await?;
        for source in crate::mining::list_adapters(&self.store, &self.mining).await? {
            if !configs.iter().any(|config| config.id == source.name) {
                configs.push(SourceConfiguration {
                    id: source.name.clone(),
                    source: source.name,
                    credential: None,
                    options: Map::new(),
                });
            }
        }
        let mut views = Vec::with_capacity(configs.len());
        for config in &configs {
            views.push(self.source_config_view(config).await);
        }
        Ok(views)
    }

    /// Show one explicit configuration or a source's implicit default.
    pub async fn show_source_config(
        &self,
        id: &str,
        mode: MemoryMode,
    ) -> Result<SourceConfigurationView> {
        Self::require_read(mode, "source_config_get")?;
        Ok(self
            .source_config_view(&self.source_config(id, None).await?)
            .await)
    }

    /// Resolve a named configuration, or an implicit default for a known source.
    pub(super) async fn source_config(
        &self,
        id: &str,
        source: Option<&str>,
    ) -> Result<SourceConfiguration> {
        let configs = self.source_configs.snapshot().await?;
        if let Some(config) = configs.iter().find(|config| config.id == id) {
            if source.is_some_and(|source| source != config.source) {
                return Err(Error::invalid_input(
                    "source_config",
                    format!(
                        "`{id}` belongs to `{}`, not `{}`",
                        config.source,
                        source.unwrap_or_default()
                    ),
                ));
            }
            return Ok(config.clone());
        }
        let name = source.unwrap_or(id);
        if id == name {
            if source.is_none()
                && crate::mining::registry::option_specs_for(&self.store, &self.mining, name)
                    .await
                    .is_err()
            {
                return Err(Error::invalid_input(
                    "source_config",
                    format!(
                        "no source or configuration `{id}`; use `memcastle source list` or `memcastle source config list`"
                    ),
                ));
            }
            return Ok(SourceConfiguration {
                id: id.into(),
                source: name.into(),
                credential: None,
                options: Map::new(),
            });
        }
        Err(Error::invalid_input(
            "source_config",
            format!(
                "no source configuration `{id}`; create it with `memcastle source config set {id} --source <source>`"
            ),
        ))
    }

    /// Change just this configuration; its options never affect another named configuration.
    pub async fn set_source_config(
        &self,
        id: &str,
        patch: SourceConfigurationPatch,
    ) -> Result<SourceConfigurationView> {
        let path = self
            .source_configs
            .path
            .as_ref()
            .ok_or_else(|| Error::MinerConfigFile {
                path: "-".into(),
                reason: "a daemon configuration file is needed to save source configurations"
                    .into(),
            })?;
        let mut state = self.source_configs.state.lock().await;
        self.source_configs.refresh(&mut state)?;
        let old = state.configs.iter().find(|config| config.id == id).cloned();
        let source = patch
            .source
            .clone()
            .or_else(|| old.as_ref().map(|config| config.source.clone()))
            .ok_or_else(|| {
                Error::invalid_input("source", "creating a source configuration needs `source`")
            })?;
        let mut config = old.unwrap_or(SourceConfiguration {
            id: id.into(),
            source: source.clone(),
            credential: None,
            options: Map::new(),
        });
        config.source = source;
        if config.id != config.source
            && crate::mining::registry::option_specs_for(&self.store, &self.mining, &config.id)
                .await
                .is_ok()
        {
            return Err(Error::invalid_input(
                "source_config",
                format!(
                    "`{id}` is already a source's implicit default configuration; use a different ID"
                ),
            ));
        }
        if patch.unset_credential {
            config.credential = None;
        }
        if patch.credential.is_some() {
            config.credential = patch.credential;
        }
        for key in patch.unset_options {
            config.options.remove(&key);
        }
        config.options.extend(patch.options);
        config
            .validate()
            .map_err(|reason| Error::invalid_input("source_config", reason))?;
        let new_options = config
            .options()
            .map_err(|reason| Error::invalid_input("options", reason))?;
        // A configuration may be written before its source is installed; if it is installed, validate its declared
        // option keys now, then leave runtime readiness checks to the miner that actually asks to use it.
        let specs =
            crate::mining::registry::option_specs_for(&self.store, &self.mining, &config.source)
                .await
                .ok();
        if let Some(specs) = &specs
            && let Some(message) = crate::domain::unknown_option(&new_options, specs)
        {
            return Err(Error::invalid_input("options", message));
        }
        let reasons = broadening_reasons(
            state.configs.iter().find(|before| before.id == id),
            &config,
            &new_options,
            specs.as_deref(),
        )?;
        if !reasons.is_empty() && !patch.allow_broaden {
            return Err(Error::invalid_input(
                "allow_broaden",
                format!(
                    "this shared configuration may mine more for every miner using it: {}; repeat with `allow_broaden = true` if intended",
                    reasons.join("; ")
                ),
            ));
        }
        let mut all = state.configs.clone();
        if let Some(entry) = all.iter_mut().find(|entry| entry.id == id) {
            *entry = config.clone();
        } else {
            all.push(config.clone());
        }
        let stamp = miners_file::write_source_configs(path, state.stamp.flatten(), &all)?;
        state.configs = all;
        state.stamp = Some(stamp);
        drop(state);
        Ok(self.source_config_view(&config).await)
    }

    /// Remove an explicitly configured instance without touching mined memories.
    pub async fn remove_source_config(&self, id: &str) -> Result<()> {
        let path = self
            .source_configs
            .path
            .as_ref()
            .ok_or_else(|| Error::MinerConfigFile {
                path: "-".into(),
                reason: "a daemon configuration file is needed".into(),
            })?;
        let mut state = self.source_configs.state.lock().await;
        self.source_configs.refresh(&mut state)?;
        if !state.configs.iter().any(|config| config.id == id) {
            return Err(Error::invalid_input(
                "source_config",
                format!("`{id}` is not explicitly configured"),
            ));
        }
        let all: Vec<_> = state
            .configs
            .iter()
            .filter(|config| config.id != id)
            .cloned()
            .collect();
        let stamp = miners_file::write_source_configs(path, state.stamp.flatten(), &all)?;
        state.configs = all;
        state.stamp = Some(stamp);
        // Removing the definition must not leave its authority behind in the credential store.
        let credentials = self.credentials.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || credentials.forget(&id))
            .await
            .map_err(|error| Error::CredentialStoreFailed {
                message: format!(
                    "forgetting the source configuration's credential was interrupted: {error}"
                ),
            })??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{OptionBreadth, OptionKind, OptionSpec};

    #[test]
    fn a_first_saved_default_requires_approval_for_an_opt_in_selection() {
        let config = SourceConfiguration {
            id: "example".into(),
            source: "example".into(),
            credential: None,
            options: Map::new(),
        };
        let specs = [OptionSpec::new("channels", "optional channels", OptionKind::String)
            .with_breadth(OptionBreadth::OptInInclude)];
        let options = crate::domain::Options::from([("channels".into(), "private".into())]);
        assert!(!broadening_reasons(None, &config, &options, Some(&specs))
            .unwrap()
            .is_empty());
        // A new named instance has no existing miners inheriting its options.
        let named = SourceConfiguration {
            id: "work".into(),
            ..config
        };
        assert!(broadening_reasons(None, &named, &options, Some(&specs))
            .unwrap()
            .is_empty());
    }
}
