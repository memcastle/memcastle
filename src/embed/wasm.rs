//! An embedding provider component has no ambient authority; one bounded store per request.

use std::io::Read;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use dashmap::DashMap;
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::config::ProviderAccount;
use crate::domain::CredentialRef;
use crate::error::{Error, Result};
use crate::plugin::embedding::Manifest;

use super::{EmbedFuture, Embedder};

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit/embedding/memcastle-embedding.wit",
        world: "embedding-provider",
    });
}

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    credential: Option<CredentialRef>,
    account: Option<ProviderAccount>,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl bindings::memcastle::embedding::host::Host for State {
    fn access_token(&mut self) -> std::result::Result<String, String> {
        if let Some(account) = &self.account {
            return crate::provider_auth::resolve(account)
                .map(|token| token.expose().to_owned())
                .map_err(|reason| {
                    format!(
                        "embedding account {reason}; check its credential reference on the daemon"
                    )
                });
        }
        match &self.credential {
            Some(CredentialRef::Env { name }) => std::env::var(name)
                .ok()
                .filter(|token| !token.is_empty())
                .ok_or_else(|| "embedding credential is unavailable".into()),
            Some(CredentialRef::File { path }) => {
                // Never read an unbounded file because a credential can name any operator-owned path.
                let mut value = String::new();
                let file =
                    std::fs::File::open(path).map_err(|_| "embedding credential is unavailable")?;
                file.take(8 * 1024)
                    .read_to_string(&mut value)
                    .map_err(|_| "embedding credential is unavailable")?;
                value
                    .lines()
                    .next()
                    .filter(|token| !token.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| "embedding credential is unavailable".into())
            }
            _ => Err("no credential was granted to this embedding provider".into()),
        }
    }
}

fn engine() -> Result<&'static Engine> {
    static ENGINE: OnceLock<std::result::Result<Engine, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut config = Config::new();
            config.epoch_interruption(true);
            let engine = Engine::new(&config).map_err(|error| error.to_string())?;
            let weak = engine.weak();
            std::thread::Builder::new()
                .name("memcastle-embedding-epoch".into())
                .spawn(move || {
                    while let Some(engine) = weak.upgrade() {
                        engine.increment_epoch();
                        drop(engine);
                        std::thread::sleep(Duration::from_millis(50));
                    }
                })
                .map_err(|error| error.to_string())?;
            Ok(engine)
        })
        .as_ref()
        .map_err(|message| Error::EmbeddingFailed {
            message: format!("embedding runtime: {message}"),
        })
}

/// Profiles can share a component; compile each verified byte generation only once.
static COMPONENTS: LazyLock<DashMap<String, Component>> = LazyLock::new(DashMap::new);

struct Inner {
    engine: &'static Engine,
    component: Component,
    manifest: Manifest,
    model: String,
    dimensions: u32,
    settings: String,
    credential: Option<CredentialRef>,
    account: Option<(String, ProviderAccount)>,
    timeout: Duration,
}

/// A selected and verified module generation, shared by search and background jobs.
#[derive(Clone)]
pub struct WasmEmbedder(Arc<Inner>);

/// Exactly one selected connection's legacy reference or shared account; never the account registry.
pub struct WasmAccess {
    /// Existing connection-local reference, when no account was selected.
    pub credential: Option<CredentialRef>,
    /// Named account selected by the connection, used only during embedding calls.
    pub account: Option<(String, ProviderAccount)>,
}

impl WasmEmbedder {
    /// Validate the component and its advertised capabilities before it is selected.
    pub fn load(
        manifest: Manifest,
        bytes: &[u8],
        model: String,
        dimensions: usize,
        settings: String,
        credential: Option<CredentialRef>,
        timeout: Duration,
    ) -> Result<Self> {
        Self::load_with_account(
            manifest,
            bytes,
            model,
            dimensions,
            settings,
            WasmAccess {
                credential,
                account: None,
            },
            timeout,
        )
    }

    /// Bind only the selected account to this component, never the entire account registry.
    pub fn load_with_account(
        manifest: Manifest,
        bytes: &[u8],
        model: String,
        dimensions: usize,
        settings: String,
        access: WasmAccess,
        timeout: Duration,
    ) -> Result<Self> {
        let WasmAccess {
            credential,
            account,
        } = access;
        if (credential.is_some() || account.is_some()) && !manifest.credential {
            return Err(Error::EmbeddingFailed {
                message: format!(
                    "embedding module `{}` did not declare credential access",
                    manifest.id
                ),
            });
        }
        let engine = engine()?;
        let digest = crate::domain::sha256_hex(bytes);
        let component = match COMPONENTS.get(&digest) {
            Some(component) => component.clone(),
            None => {
                let component =
                    Component::new(engine, bytes).map_err(|error| Error::EmbeddingFailed {
                        message: format!(
                            "embedding module `{}` is not a compatible component: {error}",
                            manifest.id
                        ),
                    })?;
                COMPONENTS.insert(digest, component.clone());
                component
            }
        };
        let dimensions = u32::try_from(dimensions).map_err(|_| Error::EmbeddingFailed {
            message: "embedding dimensions exceed the component contract".into(),
        })?;
        let provider = Self(Arc::new(Inner {
            engine,
            component,
            manifest,
            model,
            dimensions,
            settings,
            credential,
            account,
            timeout,
        }));
        provider.check()?;
        Ok(provider)
    }

    fn state(&self, granted: bool) -> State {
        let mut wasi = WasiCtxBuilder::new();
        if granted && self.0.manifest.network {
            wasi.inherit_network().allow_ip_name_lookup(true);
        }
        State {
            wasi: wasi.build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(64 * 1024 * 1024)
                .trap_on_grow_failure(true)
                .build(),
            credential: if granted {
                self.0.credential.clone()
            } else {
                None
            },
            account: if granted {
                self.0.account.as_ref().map(|(_, account)| account.clone())
            } else {
                None
            },
        }
    }

    fn instantiate(&self, granted: bool) -> Result<(Store<State>, bindings::EmbeddingProvider)> {
        let mut linker = Linker::<State>::new(self.0.engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|error| self.failed(error))?;
        bindings::EmbeddingProvider::add_to_linker::<State, HasSelf<State>>(&mut linker, |state| {
            state
        })
        .map_err(|error| self.failed(error))?;
        let mut store = Store::new(self.0.engine, self.state(granted));
        store.limiter(|state| &mut state.limits);
        store.set_epoch_deadline(
            u64::try_from((self.0.timeout.as_millis() / 50).max(1) + 1).unwrap_or(u64::MAX),
        );
        let guest =
            bindings::EmbeddingProvider::instantiate(&mut store, &self.0.component, &linker)
                .map_err(|error| self.failed(error))?;
        Ok((store, guest))
    }

    fn failed(&self, error: impl std::fmt::Display) -> Error {
        Error::EmbeddingFailed {
            message: format!(
                "embedding module `{}` failed: {}",
                self.0.manifest.id,
                error.to_string().chars().take(500).collect::<String>()
            ),
        }
    }

    fn check(&self) -> Result<()> {
        let (mut store, guest) = self.instantiate(false)?;
        let models = guest
            .memcastle_embedding_provider()
            .call_models(&mut store)
            .map_err(|error| self.failed(error))?;
        let claimed = &self.0.manifest.models;
        if models.len() != claimed.len()
            || !models.iter().all(|model| {
                claimed
                    .iter()
                    .any(|item| item.id == model.id && item.dimensions == model.dimensions)
            })
        {
            return Err(self.failed("component model capabilities differ from its manifest"));
        }
        Ok(())
    }

    fn embed_blocking(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if let Some((id, account)) = &self.0.account {
            crate::provider_auth::resolve(account).map_err(|reason| Error::EmbeddingFailed {
                message: format!(
                    "provider account `{id}` {reason}; check its credential reference on the daemon"
                ),
            })?;
        }
        let (mut store, guest) = self.instantiate(true)?;
        guest
            .memcastle_embedding_provider()
            .call_embed(
                &mut store,
                &self.0.model,
                self.0.dimensions,
                &self.0.settings,
                texts,
            )
            // Guest messages and traps can contain the token returned by access-token; keep them out of logs and jobs.
            .map_err(|_| self.failed("component trapped; check its provider configuration"))?
            .map_err(|_| {
                self.failed(
                    "component refused the request; check its model, settings and credential",
                )
            })
    }
}

impl Embedder for WasmEmbedder {
    fn embed<'a>(&'a self, texts: &'a [String]) -> EmbedFuture<'a> {
        Box::pin(async move {
            let provider = self.clone();
            let texts = texts.to_vec();
            tokio::task::spawn_blocking(move || provider.embed_blocking(&texts))
                .await
                .map_err(|error| Error::EmbeddingFailed {
                    message: format!("embedding component task failed: {error}"),
                })?
        })
    }
}
