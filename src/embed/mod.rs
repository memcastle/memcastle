//! Embedding generation: turning text into vectors, kept apart from retrieval.
//!
//! An embedding is *derived* data. This module is how MemCastle obtains one
//! — from a program the operator provides, an OpenAI-compatible HTTP
//! endpoint, an installed provider component, or not at all — and nothing here touches storage: `store` keeps
//! the vectors and SurrealDB searches them. Because the provider is
//! pluggable behind [`Embedder`], the palace works with none configured, and
//! a provider that is down degrades search to lexical instead of breaking it.
//!
//! No LLM or model is linked into MemCastle. Command providers own their credentials;
//! plugin providers receive only the selected connection's host-scoped credential reference.

pub mod command;
pub mod http;
pub mod job;
pub mod wasm;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;

use crate::config::{EmbeddingConnection, EmbeddingProvider, EmbeddingsConfig};
use crate::domain::{EMBEDDING_DIMENSION, EmbeddingSpace};
use crate::error::{Error, Result};

/// The most characters of one text sent to a provider.
///
/// Mining stores whole files (up to 256 KiB) and providers cap their input in
/// tokens, so an over-long text would make the provider reject the whole
/// batch. The head of a document carries its subject; only the vector is
/// computed from the truncated text, the drawer's content is untouched.
pub use crate::domain::MAX_EMBED_CHARS;

/// The future an [`Embedder`] returns.
pub type EmbedFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<Vec<f32>>>> + Send + 'a>>;

/// Something that turns texts into vectors.
///
/// One call embeds a batch and returns one vector per text, in order.
/// Implementations do not validate dimensions; [`Embeddings`] does, once, for
/// every provider.
pub trait Embedder: Send + Sync + 'static {
    /// Embed `texts`.
    fn embed<'a>(&'a self, texts: &'a [String]) -> EmbedFuture<'a>;
}

/// The daemon's embedding handle: a provider, or nothing.
///
/// Cheap to clone and always safe to hold: with no provider every embedding
/// request answers "not configured", which callers treat as "search
/// lexically" rather than as a failure.
#[derive(Clone, Default)]
pub struct Embeddings {
    provider: Option<Arc<dyn Embedder>>,
    batch_size: usize,
    space: Option<EmbeddingSpace>,
    profiles: Arc<BTreeMap<String, Embeddings>>,
    default_profile: Option<String>,
    installed: Arc<RwLock<BTreeMap<String, Embeddings>>>,
}

impl std::fmt::Debug for Embeddings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Nothing about the provider is printed: its configuration may hold a key.
        f.debug_struct("Embeddings")
            .field("configured", &self.provider.is_some())
            .finish()
    }
}

impl Embeddings {
    /// No provider.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Wrap a provider, sending at most `batch_size` texts per call.
    #[must_use]
    pub fn new(provider: impl Embedder, batch_size: usize) -> Self {
        Self {
            provider: Some(Arc::new(provider)),
            batch_size: batch_size.max(1),
            space: Some(EmbeddingSpace::new(
                "test".into(),
                "test".into(),
                "test".into(),
                "test".into(),
                "test".into(),
                EMBEDDING_DIMENSION,
            )),
            profiles: Arc::default(),
            default_profile: None,
            installed: Arc::default(),
        }
    }

    /// Build the handle `config` describes.
    ///
    /// # Errors
    ///
    /// [`Error::EmbeddingFailed`] if the HTTP client cannot be built.
    pub fn from_config(config: &EmbeddingsConfig) -> Result<Self> {
        let legacy = EmbeddingConnection {
            provider: config.provider,
            command: config.command.clone(),
            url: config.url.clone(),
            api_key: config.api_key.clone(),
            timeout_secs: config.timeout_secs,
            batch_size: config.batch_size,
            plugin: None,
            module: None,
            settings: serde_json::json!({}),
            credential: None,
        };
        let mut profiles = BTreeMap::new();
        if legacy.provider != EmbeddingProvider::None {
            profiles.insert(
                "default".to_string(),
                Self::from_connection(
                    "default",
                    "legacy",
                    &legacy,
                    config.model.clone().unwrap_or_default(),
                    config.dimensions,
                )?,
            );
        }
        for (name, profile) in &config.profiles {
            let connection =
                config
                    .providers
                    .get(&profile.provider)
                    .ok_or_else(|| Error::EmbeddingFailed {
                        message: format!(
                            "unknown provider `{}` for profile `{name}`",
                            profile.provider
                        ),
                    })?;
            profiles.insert(
                name.clone(),
                Self::from_connection(
                    name,
                    &profile.provider,
                    connection,
                    profile.model.clone(),
                    profile.dimensions,
                )?,
            );
        }
        let default = config.default.clone().or_else(|| {
            profiles
                .contains_key("default")
                .then(|| "default".to_string())
        });
        let mut selected = default
            .as_ref()
            .and_then(|name| profiles.get(name))
            .cloned()
            .unwrap_or_default();
        selected.profiles = Arc::new(profiles);
        selected.default_profile = default;
        Ok(selected)
    }

    /// Refresh the executable modules from the installed ledger, without swapping the scheduler's handle.
    /// A missing plugin leaves its profile unavailable while unrelated profiles keep working.
    pub fn load_plugins(
        &self,
        config: &EmbeddingsConfig,
        records: &[crate::domain::PluginRecord],
        root: &std::path::Path,
    ) -> Result<()> {
        let mut installed = BTreeMap::new();
        for (name, profile) in &config.profiles {
            let Some(connection) = config.providers.get(&profile.provider) else {
                continue;
            };
            if connection.provider != EmbeddingProvider::Plugin {
                continue;
            }
            let (Some(plugin_id), Some(module_id)) = (&connection.plugin, &connection.module)
            else {
                continue;
            };
            let Some(record) = records.iter().find(|record| &record.id == plugin_id) else {
                continue;
            };
            let Some(module) = record.manifest.modules.iter().find(|module| {
                module.kind == crate::domain::PluginModuleKind::EmbeddingProvider
                    && &module.id == module_id
            }) else {
                continue;
            };
            let base = root.join(plugin_id).join(&record.generation);
            let manifest_bytes = std::fs::read(base.join(&module.manifest))
                .map_err(|error| Error::io(module.manifest.clone(), error))?;
            let bytes = std::fs::read(base.join(&module.entry))
                .map_err(|error| Error::io(module.entry.clone(), error))?;
            if crate::domain::sha256_hex(&manifest_bytes) != module.manifest_sha256
                || crate::domain::sha256_hex(&bytes) != module.sha256
            {
                return Err(Error::EmbeddingFailed {
                    message: format!(
                        "embedding module `{module_id}` files changed; reinstall the plugin"
                    ),
                });
            }
            let descriptor = crate::plugin::embedding::parse(
                std::str::from_utf8(&manifest_bytes).map_err(|error| Error::EmbeddingFailed {
                    message: format!(
                        "embedding module `{module_id}` manifest is not UTF-8: {error}"
                    ),
                })?,
            )?;
            if descriptor.id != module.id
                || descriptor.version != module.version
                || module.contract.as_deref() != Some(descriptor.contract.as_str())
                || module.config_schema != descriptor.config_schema
            {
                return Err(Error::EmbeddingFailed {
                    message: format!(
                        "embedding module `{module_id}` differs from its installed plugin manifest; reinstall the plugin"
                    ),
                });
            }
            crate::plugin::embedding::validate_selection(
                &descriptor,
                &connection.settings,
                &profile.model,
                profile.dimensions,
            )?;
            if connection.credential.is_some() && !descriptor.credential {
                return Err(Error::EmbeddingFailed {
                    message: format!(
                        "embedding module `{module_id}` does not declare credential access"
                    ),
                });
            }
            let settings = serde_json::to_string(&connection.settings).map_err(|error| {
                Error::EmbeddingFailed {
                    message: error.to_string(),
                }
            })?;
            let provider = wasm::WasmEmbedder::load(
                descriptor,
                &bytes,
                profile.model.clone(),
                profile.dimensions,
                settings.clone(),
                connection.credential.clone(),
                std::time::Duration::from_secs(connection.timeout_secs),
            )?;
            let mut handle = Self::new(provider, connection.batch_size);
            let credential = serde_json::to_vec(&connection.credential)
                .expect("credential reference serializes");
            let identity = serde_json::to_vec(&(
                plugin_id,
                module_id,
                &module.version,
                &module.sha256,
                &module.manifest_sha256,
                &settings,
                crate::domain::sha256_hex(&credential),
            ))
            .expect("strings serialize");
            handle.space = Some(EmbeddingSpace::new(
                name.clone(),
                profile.provider.clone(),
                format!("plugin:{plugin_id}/{module_id}"),
                crate::domain::sha256_hex(&identity),
                profile.model.clone(),
                profile.dimensions,
            ));
            installed.insert(name.clone(), handle);
        }
        *self
            .installed
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = installed;
        Ok(())
    }

    fn from_connection(
        name: &str,
        provider_name: &str,
        config: &EmbeddingConnection,
        model: String,
        dimensions: usize,
    ) -> Result<Self> {
        let timeout = std::time::Duration::from_secs(config.timeout_secs);
        let kind = match config.provider {
            EmbeddingProvider::None => "none",
            EmbeddingProvider::External => "external",
            EmbeddingProvider::Command => "command",
            EmbeddingProvider::Http | EmbeddingProvider::Openai => "openai",
            EmbeddingProvider::Plugin => "plugin",
        };
        let connection = match config.provider {
            EmbeddingProvider::Command => {
                serde_json::to_string(&config.command).unwrap_or_default()
            }
            _ => config.url.clone().unwrap_or_default(),
        };
        let mut handle = match config.provider {
            EmbeddingProvider::None | EmbeddingProvider::External => Self::disabled(),
            EmbeddingProvider::Plugin => Self::disabled(),
            EmbeddingProvider::Command => Self::new(
                command::CommandEmbedder::new(config.command.clone(), Some(model.clone()), timeout)
                    .with_dimensions(dimensions),
                config.batch_size,
            ),
            EmbeddingProvider::Http | EmbeddingProvider::Openai => Self::new(
                http::HttpEmbedder::new(
                    config.url.clone().unwrap_or_default(),
                    model.clone(),
                    config.api_key.clone(),
                    timeout,
                )?
                .with_dimensions(dimensions),
                config.batch_size,
            ),
        };
        // Commands and URLs may contain credentials; the fingerprint pins them without exposing them in job rows.
        let connection_digest = crate::domain::sha256_hex(connection.as_bytes());
        handle.space = Some(EmbeddingSpace::new(
            name.into(),
            provider_name.into(),
            kind.into(),
            connection_digest,
            model,
            dimensions,
        ));
        Ok(handle)
    }

    /// Resolve a named profile; an unknown or disabled profile must never substitute another.
    pub fn select(&self, name: Option<&str>) -> Result<Self> {
        let requested = name.or(self.default_profile.as_deref());
        if let Some(provider) = requested.and_then(|name| {
            self.installed
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(name)
                .cloned()
        }) {
            return Ok(provider);
        }
        if let Some(name) = name {
            if self
                .space
                .as_ref()
                .is_some_and(|space| space.profile == name)
            {
                if self.provider.is_none()
                    && self
                        .space
                        .as_ref()
                        .is_some_and(|space| space.provider_type == "plugin")
                {
                    return Err(Error::EmbeddingProfileUnavailable { name: name.into() });
                }
                return Ok(self.clone());
            }
            let profile = self.profiles.get(name).ok_or_else(|| Error::invalid_input(
                "profile", format!("unknown embedding profile `{name}`; configure it under [embeddings.profiles.{name}]"),
            ))?;
            if profile
                .space
                .as_ref()
                .is_some_and(|space| space.provider_type == "plugin")
            {
                return Err(Error::EmbeddingProfileUnavailable { name: name.into() });
            }
            return Ok(profile.clone());
        }
        if !self.is_configured() {
            if let Some(name) = self.default_profile.as_deref()
                && self
                    .space
                    .as_ref()
                    .is_some_and(|space| space.provider_type == "plugin")
            {
                return Err(Error::EmbeddingProfileUnavailable { name: name.into() });
            }
            return Err(Error::EmbeddingsNotConfigured);
        }
        Ok(self.clone())
    }

    /// The pinned, non-secret identity of the pipeline used by this handle.
    #[must_use]
    pub fn space(&self) -> Option<&EmbeddingSpace> {
        self.space.as_ref()
    }

    /// Whether a provider is configured.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.provider.is_some()
            || self.default_profile.as_ref().is_some_and(|name| {
                self.installed
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(name)
            })
    }

    /// Embed every text, in order, validating what comes back.
    ///
    /// Texts go to the provider in batches of the configured size, each
    /// truncated to [`MAX_EMBED_CHARS`]. The result is checked once for every
    /// provider: one vector per text, each of the stored dimension and made of
    /// finite numbers. A vector that fails would otherwise be rejected by the
    /// database later, deep inside a job, with a message about an index.
    ///
    /// # Errors
    ///
    /// [`Error::EmbeddingFailed`] when no provider is configured or it fails,
    /// [`Error::EmbeddingDimension`] when it answers with the wrong length.
    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let Some(provider) = &self.provider else {
            return Err(Error::EmbeddingsNotConfigured);
        };
        let mut vectors = Vec::with_capacity(texts.len());
        for batch in texts.chunks(self.batch_size.max(1)) {
            let truncated: Vec<String> = batch.iter().map(|t| truncate(t)).collect();
            let answered = provider.embed(&truncated).await?;
            if answered.len() != batch.len() {
                return Err(Error::EmbeddingFailed {
                    message: format!(
                        "the provider returned {} vector(s) for {} text(s)",
                        answered.len(),
                        batch.len()
                    ),
                });
            }
            for vector in &answered {
                let expected = self
                    .space
                    .as_ref()
                    .map_or(EMBEDDING_DIMENSION, |s| s.dimensions);
                if vector.len() != expected {
                    return Err(Error::EmbeddingDimension {
                        expected,
                        actual: vector.len(),
                    });
                }
                if vector.iter().any(|x| !x.is_finite()) {
                    return Err(Error::EmbeddingFailed {
                        message: "the provider returned a non-finite number".to_string(),
                    });
                }
            }
            vectors.extend(answered);
        }
        Ok(vectors)
    }

    /// Embed one text.
    ///
    /// # Errors
    ///
    /// As [`Embeddings::embed`].
    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let mut vectors = self.embed(&[text.to_string()]).await?;
        vectors.pop().ok_or_else(|| Error::EmbeddingFailed {
            message: "the provider returned no vector".to_string(),
        })
    }
}

/// `text` cut to [`MAX_EMBED_CHARS`] characters, on a character boundary.
fn truncate(text: &str) -> String {
    text.chars().take(MAX_EMBED_CHARS).collect()
}

/// A deterministic fake provider for tests: the same text always yields the
/// same unit vector, and texts sharing words yield similar vectors.
///
/// Each lower-cased word hashes onto a few of the 768 coordinates, so two
/// texts that share vocabulary have a high cosine similarity — enough to test
/// that semantic ranking follows meaning-by-overlap without a model.
#[cfg(test)]
pub(crate) mod fake {
    use super::{EMBEDDING_DIMENSION, EmbedFuture, Embedder};
    use sha2::{Digest, Sha256};

    /// See the module doc.
    pub(crate) struct WordHashEmbedder;

    /// The vector [`WordHashEmbedder`] derives for `text`.
    pub(crate) fn vector_for(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; EMBEDDING_DIMENSION];
        for word in text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
        {
            let digest = Sha256::digest(word.to_lowercase().as_bytes());
            let slot = (usize::from(digest[0]) << 8 | usize::from(digest[1])) % EMBEDDING_DIMENSION;
            v[slot] += 1.0;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            v.iter_mut().for_each(|x| *x /= norm);
        } else {
            // An empty text still needs a valid, non-zero vector for cosine.
            v[0] = 1.0;
        }
        v
    }

    impl Embedder for WordHashEmbedder {
        fn embed<'a>(&'a self, texts: &'a [String]) -> EmbedFuture<'a> {
            Box::pin(async move { Ok(texts.iter().map(|t| vector_for(t)).collect()) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_profiles_keep_same_dimension_models_in_different_spaces() {
        let mut config = EmbeddingsConfig::default();
        config.providers.insert(
            "local".into(),
            EmbeddingConnection {
                provider: EmbeddingProvider::External,
                ..Default::default()
            },
        );
        config.providers.insert(
            "bulk".into(),
            EmbeddingConnection {
                provider: EmbeddingProvider::External,
                ..Default::default()
            },
        );
        config.profiles.insert(
            "local".into(),
            crate::config::EmbeddingProfileConfig {
                provider: "local".into(),
                model: "one".into(),
                dimensions: 768,
            },
        );
        config.profiles.insert(
            "bulk".into(),
            crate::config::EmbeddingProfileConfig {
                provider: "bulk".into(),
                model: "two".into(),
                dimensions: 768,
            },
        );
        config.default = Some("local".into());
        let profiles = Embeddings::from_config(&config).unwrap();
        let local = profiles.select(None).unwrap_err(); // external-only profiles do not generate vectors.
        assert!(matches!(local, Error::EmbeddingsNotConfigured));
        let local = profiles.select(Some("local")).unwrap();
        let bulk = profiles.select(Some("bulk")).unwrap();
        assert_ne!(
            local.space().unwrap().fingerprint,
            bulk.space().unwrap().fingerprint
        );
    }

    /// Returns whatever it was built with, whatever it is asked.
    struct Canned(Vec<Vec<f32>>);

    impl Embedder for Canned {
        fn embed<'a>(&'a self, _texts: &'a [String]) -> EmbedFuture<'a> {
            let vectors = self.0.clone();
            Box::pin(async move { Ok(vectors) })
        }
    }

    #[tokio::test]
    async fn embedding_without_a_provider_says_so_instead_of_returning_nothing() {
        let error = Embeddings::disabled()
            .embed_one("hello")
            .await
            .expect_err("no provider");
        assert!(matches!(error, Error::EmbeddingsNotConfigured), "{error}");
    }

    #[tokio::test]
    async fn a_vector_of_the_wrong_length_is_rejected_with_both_lengths() {
        let embeddings = Embeddings::new(Canned(vec![vec![1.0, 2.0]]), 8);
        let error = embeddings.embed_one("x").await.expect_err("wrong length");
        assert!(
            matches!(
                error,
                Error::EmbeddingDimension {
                    expected: EMBEDDING_DIMENSION,
                    actual: 2
                }
            ),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_provider_answering_with_too_few_vectors_is_rejected() {
        let embeddings = Embeddings::new(Canned(vec![]), 8);
        let error = embeddings
            .embed(&["a".to_string()])
            .await
            .expect_err("count mismatch");
        assert!(matches!(error, Error::EmbeddingFailed { .. }), "{error}");
    }

    #[tokio::test]
    async fn a_non_finite_number_is_rejected_before_it_reaches_the_index() {
        let mut vector = vec![0.0; EMBEDDING_DIMENSION];
        vector[0] = f32::NAN;
        let embeddings = Embeddings::new(Canned(vec![vector]), 8);
        assert!(embeddings.embed_one("x").await.is_err());
    }

    #[tokio::test]
    async fn texts_are_sent_in_batches_no_larger_than_the_configured_size() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Counting(Arc<AtomicUsize>);
        impl Embedder for Counting {
            fn embed<'a>(&'a self, texts: &'a [String]) -> EmbedFuture<'a> {
                assert!(texts.len() <= 2, "batch of {}", texts.len());
                self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move { Ok(texts.iter().map(|t| fake::vector_for(t)).collect()) })
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let embeddings = Embeddings::new(Counting(calls.clone()), 2);
        let texts: Vec<String> = (0..5).map(|i| format!("text {i}")).collect();
        let vectors = embeddings.embed(&texts).await.expect("embeds");
        assert_eq!(vectors.len(), 5);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn a_text_is_truncated_on_a_character_boundary() {
        let long = "é".repeat(MAX_EMBED_CHARS + 10);
        assert_eq!(truncate(&long).chars().count(), MAX_EMBED_CHARS);
    }

    #[test]
    fn the_debug_form_reveals_nothing_about_the_provider() {
        let text = format!("{:?}", Embeddings::new(fake::WordHashEmbedder, 4));
        assert_eq!(text, "Embeddings { configured: true }");
    }
}
