//! The `http` embedding provider: an OpenAI-compatible `/embeddings` endpoint.
//!
//! OpenAI, Ollama, llama.cpp's server and vLLM all speak this shape, so one
//! implementation covers hosted and local models. A local server needs no key;
//! a hosted one gets its key as a bearer token from `config::Secret`, which
//! never reaches a log line or an error message.

use std::time::Duration;

use serde::Deserialize;

use super::{EmbedFuture, Embedder};
use crate::config::{ProviderAccount, provider_accounts::ProviderAuth};
use crate::domain::{EMBEDDING_DIMENSION, Secret};
use crate::error::{Error, Result};

/// Calls `POST {base_url}/embeddings`.
pub struct HttpEmbedder {
    client: reqwest::Client,
    endpoint: String,
    model: String,
    api_key: Option<Secret>,
    account: Option<(String, ProviderAccount)>,
    dimensions: usize,
}

#[derive(Deserialize)]
struct Response {
    data: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(default)]
    index: usize,
    embedding: Vec<f32>,
}

impl HttpEmbedder {
    /// A provider for the API at `base_url` (no trailing `/embeddings`).
    ///
    /// # Errors
    ///
    /// [`Error::EmbeddingFailed`] if the HTTP client cannot be built.
    pub fn new(
        base_url: String,
        model: String,
        api_key: Option<Secret>,
        timeout: Duration,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|source| Error::EmbeddingFailed {
                message: format!("could not build the HTTP client: {source}"),
            })?;
        Ok(Self {
            client,
            endpoint: format!("{}/embeddings", base_url.trim_end_matches('/')),
            model,
            api_key,
            account: None,
            dimensions: EMBEDDING_DIMENSION,
        })
    }

    /// Request the selected profile's vector length, not the legacy palace length.
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// Resolve a shared account for each request so secret rotation needs no restart.
    #[must_use]
    pub fn with_account(mut self, id: String, account: ProviderAccount) -> Self {
        self.account = Some((id, account));
        self
    }

    async fn run(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut request = self.client.post(&self.endpoint).json(&serde_json::json!({
            "model": self.model,
            "input": texts,
            // Lets a model with a longer native vector (OpenAI's v3 family)
            // return the dimension the palace stores; servers that do not
            // support it ignore the field.
            "dimensions": self.dimensions,
        }));
        if let Some((id, account)) = &self.account {
            let key = crate::provider_auth::resolve(account).map_err(|reason| Error::EmbeddingFailed {
                message: format!("provider account `{id}` {reason}; check its credential reference in the daemon environment or file"),
            })?;
            request = match &account.auth {
                ProviderAuth::Bearer { .. } => request.bearer_auth(key.expose()),
                ProviderAuth::ApiKey { header, .. } => {
                    request.header(header.as_deref().unwrap_or("x-api-key"), key.expose())
                }
            };
        } else if let Some(key) = &self.api_key {
            request = request.bearer_auth(key.expose());
        }
        // `without_url`: the message must not echo a URL that may carry
        // credentials in its userinfo.
        let response = request
            .send()
            .await
            .map_err(|source| Error::EmbeddingFailed {
                message: format!("the request failed: {}", source.without_url()),
            })?;
        let status = response.status();
        if !status.is_success() {
            // A remote endpoint can echo the request's token; never surface its body for authenticated calls.
            let body = if self.account.is_some() || self.api_key.is_some() {
                "check the provider's account credential and endpoint".to_owned()
            } else {
                response
                    .text()
                    .await
                    .unwrap_or_default()
                    .trim()
                    .chars()
                    .take(300)
                    .collect()
            };
            return Err(Error::EmbeddingFailed {
                message: format!("the endpoint answered {status}: {body}"),
            });
        }
        let mut parsed: Response =
            response
                .json()
                .await
                .map_err(|source| Error::EmbeddingFailed {
                    message: format!(
                        "the endpoint's answer is not an OpenAI-style embeddings response: {}",
                        source.without_url()
                    ),
                })?;
        // The API numbers its items; a server is free to reorder them.
        parsed.data.sort_by_key(|item| item.index);
        Ok(parsed.data.into_iter().map(|item| item.embedding).collect())
    }
}

impl Embedder for HttpEmbedder {
    fn embed<'a>(&'a self, texts: &'a [String]) -> EmbedFuture<'a> {
        Box::pin(self.run(texts))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::Json;
    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use serde_json::{Value, json};

    use super::*;

    /// What the stub saw: the authorization header and the JSON body.
    type Seen = Arc<Mutex<Vec<(Option<String>, Value)>>>;

    /// A local OpenAI-style server answering `answer` for every request.
    async fn stub(status: StatusCode, answer: Value) -> (String, Seen) {
        let seen: Seen = Arc::default();
        let state = (seen.clone(), status, answer);
        let app = Router::new()
            .route(
                "/v1/embeddings",
                post(
                    |State((seen, status, answer)): State<(Seen, StatusCode, Value)>,
                     headers: HeaderMap,
                     Json(body): Json<Value>| async move {
                        let auth = headers
                            .get("authorization")
                            .or_else(|| headers.get("x-vendor-key"))
                            .map(|v| v.to_str().unwrap_or_default().to_string());
                        seen.lock().unwrap().push((auth, body));
                        (status, Json(answer))
                    },
                ),
            )
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://127.0.0.1:{port}/v1"), seen)
    }

    fn vector(value: f32) -> Vec<f32> {
        vec![value; EMBEDDING_DIMENSION]
    }

    #[tokio::test]
    async fn vectors_come_back_in_input_order_even_when_the_server_reorders_them() {
        let answer = json!({"data": [
            {"index": 1, "embedding": vector(2.0)},
            {"index": 0, "embedding": vector(1.0)},
        ]});
        let (url, _) = stub(StatusCode::OK, answer).await;
        let embedder = HttpEmbedder::new(url, "m".into(), None, Duration::from_secs(10)).unwrap();
        let vectors = embedder
            .run(&["a".to_string(), "b".to_string()])
            .await
            .expect("answers");
        assert_eq!(vectors[0][0], 1.0);
        assert_eq!(vectors[1][0], 2.0);
    }

    #[tokio::test]
    async fn the_request_names_the_model_and_the_stored_dimension() {
        let answer = json!({"data": [{"index": 0, "embedding": vector(1.0)}]});
        let (url, seen) = stub(StatusCode::OK, answer).await;
        let embedder =
            HttpEmbedder::new(url, "nomic".into(), None, Duration::from_secs(10)).unwrap();
        embedder.run(&["a".to_string()]).await.expect("answers");
        let (_, body) = seen.lock().unwrap()[0].clone();
        assert_eq!(body["model"], "nomic");
        assert_eq!(body["dimensions"], 768);
        assert_eq!(body["input"], json!(["a"]));
    }

    #[tokio::test]
    async fn a_profile_can_request_a_different_dimension_from_the_same_protocol() {
        let (url, seen) = stub(
            StatusCode::OK,
            json!({"data": [{"index": 0, "embedding": [1.0, 0.0, 0.0]}]}),
        )
        .await;
        let embedder = HttpEmbedder::new(url, "small".into(), None, Duration::from_secs(10))
            .unwrap()
            .with_dimensions(3);
        let vectors = embedder.run(&["a".into()]).await.unwrap();
        assert_eq!(vectors[0].len(), 3);
        assert_eq!(seen.lock().unwrap()[0].1["dimensions"], 3);
    }

    #[tokio::test]
    async fn two_named_openai_connections_send_the_selected_model_to_their_own_endpoint() {
        let (local, local_seen) = stub(
            StatusCode::OK,
            json!({"data": [{"index": 0, "embedding": [1.0, 0.0, 0.0]}]}),
        )
        .await;
        let (hosted, hosted_seen) = stub(
            StatusCode::OK,
            json!({"data": [{"index": 0, "embedding": [0.0, 1.0, 0.0]}]}),
        )
        .await;
        let mut config = crate::config::EmbeddingsConfig::default();
        for (name, url, model) in [
            ("local", local, "ollama-model"),
            ("hosted", hosted, "hosted-model"),
        ] {
            config.providers.insert(
                name.into(),
                crate::config::EmbeddingConnection {
                    provider: crate::config::EmbeddingProvider::Openai,
                    url: Some(url),
                    ..Default::default()
                },
            );
            config.profiles.insert(
                name.into(),
                crate::config::EmbeddingProfileConfig {
                    provider: name.into(),
                    model: model.into(),
                    dimensions: 3,
                },
            );
        }
        config.default = Some("local".into());
        let profiles = super::super::Embeddings::from_config(&config).unwrap();
        assert_eq!(
            profiles
                .select(None)
                .unwrap()
                .embed_one("hello")
                .await
                .unwrap(),
            [1.0, 0.0, 0.0]
        );
        assert_eq!(
            profiles
                .select(Some("hosted"))
                .unwrap()
                .embed_one("hello")
                .await
                .unwrap(),
            [0.0, 1.0, 0.0]
        );
        assert_eq!(local_seen.lock().unwrap()[0].1["model"], "ollama-model");
        assert_eq!(hosted_seen.lock().unwrap()[0].1["model"], "hosted-model");
        assert_ne!(
            profiles
                .select(Some("local"))
                .unwrap()
                .space()
                .unwrap()
                .fingerprint,
            profiles
                .select(Some("hosted"))
                .unwrap()
                .space()
                .unwrap()
                .fingerprint
        );
    }

    #[tokio::test]
    async fn a_key_is_sent_as_a_bearer_token_only_when_configured() {
        let answer = json!({"data": [{"index": 0, "embedding": vector(1.0)}]});
        let (url, seen) = stub(StatusCode::OK, answer).await;
        let keyed = HttpEmbedder::new(
            url.clone(),
            "m".into(),
            Some(Secret::new("sk-test")),
            Duration::from_secs(10),
        )
        .unwrap();
        keyed.run(&["a".to_string()]).await.unwrap();
        let keyless = HttpEmbedder::new(url, "m".into(), None, Duration::from_secs(10)).unwrap();
        keyless.run(&["a".to_string()]).await.unwrap();

        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].0.as_deref(), Some("Bearer sk-test"));
        assert_eq!(
            seen[1].0, None,
            "a local server must not get an empty bearer"
        );
    }

    #[tokio::test]
    async fn two_connections_share_one_account_and_read_a_rotated_file_on_each_request() {
        let (url, seen) = stub(
            StatusCode::OK,
            json!({"data": [{"index": 0, "embedding": [1.0, 0.0, 0.0]}]}),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider-key");
        std::fs::write(&path, "first-key\n").unwrap();
        let account: ProviderAccount = toml::from_str(&format!(
            "provider_id = 'openai'\nauth = {{ type = 'bearer', credential = {{ type = 'file', path = {:?} }} }}",
            path.display().to_string()
        )).unwrap();
        let mut config = crate::config::EmbeddingsConfig::default();
        for id in ["primary", "secondary"] {
            config.providers.insert(
                id.into(),
                crate::config::EmbeddingConnection {
                    provider: crate::config::EmbeddingProvider::Openai,
                    url: Some(url.clone()),
                    provider_id: Some("openai".into()),
                    account: Some("shared".into()),
                    ..Default::default()
                },
            );
            config.profiles.insert(
                id.into(),
                crate::config::EmbeddingProfileConfig {
                    provider: id.into(),
                    model: "model".into(),
                    dimensions: 3,
                },
            );
        }
        let accounts = std::iter::once(("shared".into(), account)).collect();
        let profiles =
            super::super::Embeddings::from_config_with_accounts(&config, &accounts).unwrap();
        let first_space = profiles
            .select(Some("primary"))
            .unwrap()
            .space()
            .unwrap()
            .fingerprint
            .clone();
        profiles
            .select(Some("primary"))
            .unwrap()
            .embed_one("one")
            .await
            .unwrap();
        profiles
            .select(Some("secondary"))
            .unwrap()
            .embed_one("two")
            .await
            .unwrap();
        std::fs::write(&path, "second-key\n").unwrap();
        profiles
            .select(Some("primary"))
            .unwrap()
            .embed_one("three")
            .await
            .unwrap();
        let other = dir.path().join("other-provider-key");
        std::fs::write(&other, "budget-key").unwrap();
        let other_account: ProviderAccount = toml::from_str(&format!(
            "provider_id = 'openai'\nauth = {{ type = 'bearer', credential = {{ type = 'file', path = {:?} }} }}",
            other.display().to_string()
        )).unwrap();
        let other_connection = HttpEmbedder::new(url, "model".into(), None, Duration::from_secs(5))
            .unwrap()
            .with_account("budget".into(), other_account);
        other_connection.run(&["four".into()]).await.unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(
            seen.iter()
                .map(|(header, _)| header.as_deref())
                .collect::<Vec<_>>(),
            [
                Some("Bearer first-key"),
                Some("Bearer first-key"),
                Some("Bearer second-key"),
                Some("Bearer budget-key")
            ]
        );
        assert_eq!(
            first_space,
            profiles
                .select(Some("primary"))
                .unwrap()
                .space()
                .unwrap()
                .fingerprint
        );
    }

    #[tokio::test]
    async fn a_key_header_uses_only_the_selected_account_and_missing_secrets_are_redacted() {
        let (url, seen) = stub(
            StatusCode::OK,
            json!({"data": [{"index": 0, "embedding": vector(1.0)}]}),
        )
        .await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        std::fs::write(&path, "key-value").unwrap();
        let account: ProviderAccount = toml::from_str(&format!(
            "provider_id = 'vendor'\nauth = {{ type = 'api_key', header = 'x-vendor-key', credential = {{ type = 'file', path = {:?} }} }}",
            path.display().to_string()
        )).unwrap();
        let provider = HttpEmbedder::new(url, "model".into(), None, Duration::from_secs(5))
            .unwrap()
            .with_account("chosen".into(), account);
        provider.run(&["one".into()]).await.unwrap();
        assert_eq!(seen.lock().unwrap()[0].0.as_deref(), Some("key-value"));
        std::fs::remove_file(&path).unwrap();
        let error = provider.run(&["two".into()]).await.unwrap_err().to_string();
        assert!(error.contains("chosen"));
        assert!(!error.contains("key-value"));
        assert!(!error.contains(path.to_str().unwrap()));
    }

    #[tokio::test]
    async fn an_error_status_is_reported_with_the_servers_message_and_never_the_key() {
        let (url, _) = stub(StatusCode::UNAUTHORIZED, json!({"error": "sk-secret"})).await;
        let embedder = HttpEmbedder::new(
            url,
            "m".into(),
            Some(Secret::new("sk-secret")),
            Duration::from_secs(10),
        )
        .unwrap();
        let message = embedder
            .run(&["a".to_string()])
            .await
            .expect_err("401")
            .to_string();
        assert!(message.contains("401"), "{message}");
        assert!(
            message.contains("check the provider's account credential"),
            "{message}"
        );
        assert!(!message.contains("sk-secret"), "{message}");
    }

    #[tokio::test]
    async fn an_unreachable_endpoint_is_a_failure_not_a_panic() {
        let embedder = HttpEmbedder::new(
            "http://127.0.0.1:1/v1".into(),
            "m".into(),
            None,
            Duration::from_secs(2),
        )
        .unwrap();
        let error = embedder.run(&["a".to_string()]).await.expect_err("refused");
        assert!(matches!(error, Error::EmbeddingFailed { .. }), "{error}");
    }
}
