//! The pure, versioned embedding-module descriptor; no runtime or storage access.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::is_valid_source_name;
use crate::error::{Error, Result};

/// The WIT package implemented by embedding provider components.
pub const CONTRACT: &str = "0.1.0";

/// A model and the dimensions this implementation actually supports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Identifier sent with each request to this module.
    pub id: String,
    /// Allowed output dimensions for this model.
    pub dimensions: Vec<u32>,
}

/// Module-owned capabilities and execution permissions, checked before publication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Stable module identifier.
    pub id: String,
    /// Independently released module version.
    pub version: String,
    /// Implemented WIT interface version.
    pub contract: String,
    /// Advertised capabilities checked against the component at load.
    pub models: Vec<Model>,
    #[serde(default)]
    /// Whether this component may open network connections.
    pub network: bool,
    #[serde(default)]
    /// Whether the host may hand it the selected connection's token.
    pub credential: bool,
    #[serde(default)]
    /// Restricted, explicit schema for non-secret instance settings.
    pub config_schema: Option<Value>,
}

impl Manifest {
    /// A user-visible digest of the permissions, never a digest of a credential value.
    pub fn consent_digest(&self) -> String {
        let bytes = serde_json::to_vec(&(&self.id, self.network, self.credential))
            .expect("module permissions serialize");
        crate::domain::sha256_hex(&bytes)
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::PluginManifestInvalid {
        message: message.into(),
    }
}

/// Parse a module manifest without granting anything to its component.
pub fn parse(text: &str) -> Result<Manifest> {
    let manifest: Manifest = toml::from_str(text)
        .map_err(|error| invalid(format!("embedding provider manifest: {error}")))?;
    if !is_valid_source_name(&manifest.id) {
        return Err(invalid(
            "embedding provider needs a lowercase hyphenated id",
        ));
    }
    semver::Version::parse(&manifest.version)
        .map_err(|error| invalid(format!("embedding provider version: {error}")))?;
    if manifest.contract != CONTRACT {
        return Err(invalid(format!(
            "embedding provider `{}` needs contract {CONTRACT}; found {}",
            manifest.id, manifest.contract
        )));
    }
    if manifest.models.is_empty() || manifest.models.len() > 64 {
        return Err(invalid("embedding provider needs 1..=64 advertised models"));
    }
    let mut names = HashSet::new();
    for model in &manifest.models {
        if model.id.trim().is_empty()
            || model.id.len() > 256
            || !names.insert(&model.id)
            || model.dimensions.is_empty()
            || model.dimensions.len() > 32
            || model
                .dimensions
                .iter()
                .any(|dimension| *dimension == 0 || *dimension > 65_536)
            || model.dimensions.iter().collect::<HashSet<_>>().len() != model.dimensions.len()
        {
            return Err(invalid(format!(
                "embedding provider `{}` has an invalid or duplicate model/dimension declaration",
                manifest.id
            )));
        }
    }
    if let Some(schema) = &manifest.config_schema {
        validate_schema(schema)?;
    }
    Ok(manifest)
}

/// Reject unknown schema vocabulary instead of silently accepting settings the author meant to constrain.
fn validate_schema(schema: &Value) -> Result<()> {
    let object = schema
        .as_object()
        .ok_or_else(|| invalid("embedding config_schema must be an object"))?;
    if object.get("type").and_then(Value::as_str) != Some("object") {
        return Err(invalid(
            "embedding config_schema must declare type = object",
        ));
    }
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type" | "properties" | "required" | "additionalProperties"
        ) {
            return Err(invalid(format!(
                "unsupported embedding config_schema keyword `{key}`"
            )));
        }
    }
    if let Some(properties) = object.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| invalid("embedding config_schema properties must be an object"))?;
        for (name, property) in properties {
            if secret_field(name) {
                return Err(invalid(format!(
                    "embedding setting `{name}` must be a credential reference, not a schema setting"
                )));
            }
            let property = property.as_object().ok_or_else(|| {
                invalid(format!(
                    "embedding setting `{name}` schema must be an object"
                ))
            })?;
            if property.len() != 1
                || !matches!(
                    property.get("type").and_then(Value::as_str),
                    Some("string" | "number" | "integer" | "boolean")
                )
            {
                return Err(invalid(format!(
                    "embedding setting `{name}` needs a supported scalar type"
                )));
            }
        }
    }
    if let Some(required) = object.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| invalid("embedding config_schema required must be an array"))?;
        if required.iter().any(|name| {
            name.as_str().is_none_or(|name| {
                object
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_none_or(|props| !props.contains_key(name))
            })
        }) {
            return Err(invalid(
                "embedding config_schema required contains an unknown property",
            ));
        }
    }
    if object.get("additionalProperties") == Some(&Value::Bool(true)) {
        return Err(invalid(
            "embedding config_schema cannot allow unspecified settings",
        ));
    }
    if object
        .get("additionalProperties")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(invalid(
            "embedding config_schema additionalProperties must be false",
        ));
    }
    Ok(())
}

/// Validate instance settings and selected profile against the advertised contract.
pub fn validate_selection(
    manifest: &Manifest,
    settings: &Value,
    model: &str,
    dimensions: usize,
) -> Result<()> {
    if !manifest.models.iter().any(|item| {
        item.id == model
            && item
                .dimensions
                .iter()
                .any(|size| usize::try_from(*size).ok() == Some(dimensions))
    }) {
        return Err(invalid(format!(
            "embedding provider `{}` does not support model `{model}` at {dimensions} dimensions",
            manifest.id
        )));
    }
    let settings = settings
        .as_object()
        .ok_or_else(|| invalid("embedding settings must be an object"))?;
    let Some(schema) = manifest.config_schema.as_ref() else {
        return if settings.is_empty() {
            Ok(())
        } else {
            Err(invalid("embedding provider accepts no settings"))
        };
    };
    let properties = schema.get("properties").and_then(Value::as_object);
    for (name, value) in settings {
        if secret_field(name) {
            return Err(invalid(format!(
                "embedding setting `{name}` must be supplied as a credential reference"
            )));
        }
        let Some(kind) = properties
            .and_then(|items| items.get(name))
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str)
        else {
            return Err(invalid(format!("unknown embedding setting `{name}`")));
        };
        if !match kind {
            "string" => value.is_string(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            _ => false,
        } {
            return Err(invalid(format!(
                "embedding setting `{name}` needs a {kind}"
            )));
        }
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !settings.contains_key(name) {
                return Err(invalid(format!("embedding setting `{name}` is required")));
            }
        }
    }
    Ok(())
}

fn secret_field(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "credential",
        "authorization",
    ]
    .iter()
    .any(|word| name.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> Manifest {
        parse("id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3, 768]\n").unwrap()
    }

    #[test]
    fn an_unsupported_model_or_dimension_is_rejected_before_any_component_call() {
        let manifest = descriptor();
        assert!(validate_selection(&manifest, &serde_json::json!({}), "other", 3).is_err());
        assert!(validate_selection(&manifest, &serde_json::json!({}), "one", 1536).is_err());
        validate_selection(&manifest, &serde_json::json!({}), "one", 768).unwrap();
    }

    #[test]
    fn unknown_schema_vocabulary_is_not_silently_ignored() {
        let mut manifest = descriptor();
        manifest.config_schema = Some(
            serde_json::json!({"type": "object", "properties": {"region": {"type": "string"}}, "required": ["region"], "additionalProperties": false}),
        );
        assert!(validate_selection(&manifest, &serde_json::json!({}), "one", 3).is_err());
        assert!(
            validate_selection(&manifest, &serde_json::json!({"region": 7}), "one", 3).is_err()
        );
        validate_selection(&manifest, &serde_json::json!({"region": "eu"}), "one", 3).unwrap();
        let parsed = parse("id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n[config_schema]\ntype = 'object'\nrequired = ['region']\nadditionalProperties = false\n[config_schema.properties.region]\ntype = 'string'\n").unwrap();
        validate_selection(&parsed, &serde_json::json!({"region": "eu"}), "one", 3).unwrap();
        assert!(parse("id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n[config_schema]\ntype = 'object'\nformat = 'unvalidated'\n").is_err());
    }

    #[test]
    fn permission_changes_need_new_consent_without_exposing_a_credential() {
        let mut manifest = descriptor();
        let original = manifest.consent_digest();
        manifest.network = true;
        assert_ne!(original, manifest.consent_digest());
        manifest.credential = true;
        assert_ne!(original, manifest.consent_digest());
    }

    #[test]
    fn malformed_capabilities_and_secret_bearing_schemas_are_rejected_before_installation() {
        let bad = [
            (
                "id = 'Not-a-module'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n",
                "lowercase",
            ),
            (
                "id = 'synthetic'\nversion = 'no-version'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n",
                "version",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '9.0.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n",
                "contract",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n",
                "models",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3, 3]\n",
                "duplicate model/dimension",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [0]\n",
                "invalid",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n[[models]]\nid = 'one'\ndimensions = [4]\n",
                "duplicate",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n[config_schema]\ntype = 'object'\nadditionalProperties = true\n",
                "unspecified settings",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n[config_schema]\ntype = 'object'\n[config_schema.properties.api_key]\ntype = 'string'\n",
                "credential reference",
            ),
            (
                "id = 'synthetic'\nversion = '0.1.0'\ncontract = '0.1.0'\n[[models]]\nid = 'one'\ndimensions = [3]\n[config_schema]\ntype = 'object'\nrequired = ['region']\n",
                "unknown property",
            ),
        ];
        for (text, expected) in bad {
            let error =
                parse(text).expect_err("invalid capabilities must never become installable");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn instance_settings_must_match_the_declared_types_and_cannot_embed_a_token() {
        let mut manifest = descriptor();
        manifest.config_schema = Some(serde_json::json!({
            "type": "object", "additionalProperties": false,
            "properties": {"region": {"type": "string"}, "parallel": {"type": "integer"}, "active": {"type": "boolean"}}
        }));
        for settings in [
            serde_json::json!(["not an object"]),
            serde_json::json!({"api_key": "never-log-this"}),
            serde_json::json!({"unknown": 1}),
            serde_json::json!({"parallel": 1.5}),
            serde_json::json!({"active": "true"}),
        ] {
            assert!(
                validate_selection(&manifest, &settings, "one", 3).is_err(),
                "{settings}"
            );
        }
        validate_selection(
            &manifest,
            &serde_json::json!({"region": "eu", "parallel": 2, "active": true}),
            "one",
            3,
        )
        .unwrap();
    }
}
