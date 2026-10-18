//! A vector's identity is its generation pipeline, never its length alone.

use serde::{Deserialize, Serialize};

use super::{content_hash, sha256_hex};

/// Increment when the text sent to providers or its normalization changes.
pub const PIPELINE_VERSION: u32 = 1;
/// Characters of drawer content sent to an embedding provider.
pub const MAX_EMBED_CHARS: usize = 8_000;

/// The non-secret settings pinned by a job and copied to each resulting vector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingSpace {
    /// Human-selectable alias; not sufficient by itself to identify compatibility.
    pub profile: String,
    /// Name of the configured connection.
    pub provider: String,
    /// Provider protocol or command implementation.
    pub provider_type: String,
    /// Digest of non-persistable connection details; never an endpoint or command argument.
    pub connection: String,
    /// Model identifier sent to the provider.
    pub model: String,
    /// Expected vector length.
    pub dimensions: usize,
    /// Revision of the input normalization and truncation pipeline.
    pub pipeline_version: u32,
    /// Stable compatibility identity calculated from all generation-affecting settings.
    pub fingerprint: String,
}

impl EmbeddingSpace {
    /// Include connection identity because two endpoints may serve different models under the same name.
    pub fn new(
        profile: String,
        provider: String,
        provider_type: String,
        connection: String,
        model: String,
        dimensions: usize,
    ) -> Self {
        let identity = serde_json::to_vec(&(
            provider.as_str(),
            provider_type.as_str(),
            connection.as_str(),
            model.as_str(),
            dimensions,
            PIPELINE_VERSION,
            MAX_EMBED_CHARS,
        ))
        .expect("a tuple of strings and numbers always serializes");
        let fingerprint = sha256_hex(&identity);
        Self {
            profile,
            provider,
            provider_type,
            connection,
            model,
            dimensions,
            pipeline_version: PIPELINE_VERSION,
            fingerprint,
        }
    }

    /// A hash of precisely the input passed to the provider; canonical content remains unchanged.
    pub fn input_hash(text: &str) -> String {
        content_hash(&text.chars().take(MAX_EMBED_CHARS).collect::<String>())
    }
}

/// An explicit maintenance job may refresh vectors; automatic sweeps never do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingRefresh {
    /// Only fill vectors absent from the selected space.
    #[default]
    Missing,
    /// Fill absent vectors and regenerate those with out-of-date input hashes.
    Stale,
    /// Recompute every vector in the selected scope.
    All,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_connection_or_model_changes_the_space_even_at_the_same_dimension() {
        let make = |connection: &str, model: &str| {
            EmbeddingSpace::new(
                "local".into(),
                "local".into(),
                "openai".into(),
                connection.into(),
                model.into(),
                768,
            )
        };
        assert_ne!(
            make("a", "model").fingerprint,
            make("b", "model").fingerprint
        );
        assert_ne!(
            make("a", "model").fingerprint,
            make("a", "another").fingerprint
        );
    }

    #[test]
    fn the_input_hash_tracks_the_truncated_text_not_the_unembedded_tail() {
        let prefix = "a".repeat(MAX_EMBED_CHARS);
        assert_eq!(
            EmbeddingSpace::input_hash(&prefix),
            EmbeddingSpace::input_hash(&(prefix.clone() + "tail"))
        );
        assert_ne!(
            EmbeddingSpace::input_hash("a"),
            EmbeddingSpace::input_hash("b")
        );
    }
}
