//! Named, reusable settings for a source, independent of a miner's destination or timetable.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{CredentialRef, Options, is_valid_miner_name};

/// A source's default configuration has the source's own ID; other instances have separate IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfiguration {
    /// Unique configuration ID, never a miner ID.
    pub id: String,
    /// Installed source adapter.
    pub source: String,
    /// Optional reference to a credential, never secret contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<CredentialRef>,
    /// Source-native option defaults, never a miner mapping or schedule.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub options: Map<String, Value>,
}

impl SourceConfiguration {
    /// Validate without I/O; source availability and option specs are checked at activation.
    pub fn validate(&self) -> Result<(), String> {
        if !is_valid_miner_name(&self.id) {
            return Err(format!(
                "source configuration `{}` needs a lowercase slug ID",
                self.id
            ));
        }
        if self.source.trim().is_empty() {
            return Err(format!("source configuration `{}` needs `source`", self.id));
        }
        if let Some(CredentialRef::Env { name }) = &self.credential
            && name.is_empty()
        {
            return Err("`credential.name` cannot be empty".into());
        }
        if let Some(CredentialRef::File { path }) = &self.credential
            && path.is_empty()
        {
            return Err("`credential.path` cannot be empty".into());
        }
        self.options()?;
        Ok(())
    }

    /// Flatten source options for the existing adapter contract, refusing secret-looking keys.
    pub fn options(&self) -> Result<Options, String> {
        super::miner::check_open_table("options", &self.options)?;
        let mut options = Options::new();
        for (key, value) in &self.options {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Bool(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                Value::Array(values) if values.iter().all(Value::is_string) => values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","),
                _ => return Err(format!("`options.{key}` needs a scalar or list of strings")),
            };
            options.insert(key.clone(), text);
        }
        Ok(options)
    }
}

/// Every ID has exactly one source configuration, even across different sources.
pub fn validate_source_configs(configs: &[SourceConfiguration]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for config in configs {
        config.validate()?;
        if !seen.insert(&config.id) {
            return Err(format!(
                "source configuration `{}` is defined twice",
                config.id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_source_configuration_keeps_secrets_out_of_every_option_key() {
        let mut config = SourceConfiguration {
            id: "work".into(),
            source: "directory".into(),
            credential: None,
            options: Map::new(),
        };
        config
            .options
            .insert("private_key".into(), Value::String("do-not-store".into()));
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("looks like a secret")
        );
        config.options.clear();
        config.options.insert(
            "include".into(),
            serde_json::json!(["docs/**", "README.md"]),
        );
        assert_eq!(config.options().unwrap()["include"], "docs/**,README.md");
    }
}
