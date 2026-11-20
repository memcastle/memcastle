//! The WebAssembly source runtime: a [`SourceAdapter`] whose methods run a component (docs/adr/026).
//!
//! [`WasmAdapter`] is the same logical contract as the built-in adapters, so the pipeline cannot tell them apart.
//! What this module adds is the isolation: a component runs in a fresh sandbox per call, with the filesystem, the
//! environment, the network and programs available only as far as its manifest was agreed to, a memory ceiling, and
//! a time limit enforced by the engine rather than by the source's good manners.
//!
//! This is the only source module that names `wasmtime` (`tests/source_isolation.rs`), and like every adapter it never
//! touches the store or the jobs.

mod host;
mod process;
mod reconciliation;

use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use dashmap::DashMap;
use futures::future::BoxFuture;
use wasmtime::component::{Accessor, Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, Trap};

use crate::config::MiningConfig;
use crate::domain::{
    AccessTokens, Candidate, CanonicalDocument, Cursor, Options, Permissions, RawDocument, Segment,
    SourceCapabilities, SourceKind, SourceManifest, SourceRef, sha256_hex,
};
use crate::error::{Error, Result};
use crate::source::manifest::check_compatible;

use super::adapter::{Discovery, SourceAdapter};
use host::memcastle::source::types as wit;
use host::{CredentialSelection, HostState, Source, SourcePre, TokenGrant};

/// How often the engine's epoch advances, which is the resolution of a source's time limit.
const EPOCH_TICK: Duration = Duration::from_millis(50);

/// The longest a message from a failed source is kept, so a trap's backtrace cannot flood a job's error.
const MAX_MESSAGE_CHARS: usize = 600;

type GuestResult<T> = wasmtime::Result<std::result::Result<T, wit::SourceError>>;

/// The one engine every source runs on. Epoch interruption preempts even a guest that never calls the host,
/// so the async executor can enforce its wall-clock deadline and keep other jobs moving.
fn engine() -> Result<&'static Engine> {
    static ENGINE: OnceLock<std::result::Result<Engine, String>> = OnceLock::new();
    let engine = ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.epoch_interruption(true);
        config.wasm_component_model_async(true);
        let engine = Engine::new(&config).map_err(|e| format!("{e:#}"))?;
        let weak = engine.weak();
        std::thread::Builder::new()
            .name("memcastle-source-epoch".to_string())
            .spawn(move || {
                // Ends with the engine, which a process-wide static never does; the check is for tidiness.
                while let Some(engine) = weak.upgrade() {
                    engine.increment_epoch();
                    drop(engine);
                    std::thread::sleep(EPOCH_TICK);
                }
            })
            .map_err(|e| format!("cannot start the source timer: {e}"))?;
        Ok(engine)
    });
    engine.as_ref().map_err(|message| Error::SourceFailed {
        name: "runtime".to_string(),
        message: message.clone(),
    })
}

/// Compiled components by the digest of their bytes: compiling is the expensive step, and the same installed source
/// is loaded for every job that mines it.
static COMPONENTS: LazyLock<DashMap<String, Component>> = LazyLock::new(DashMap::new);

/// A source loaded from a WebAssembly component, behind the [`SourceAdapter`] contract.
#[derive(Clone)]
pub struct WasmAdapter {
    inner: Arc<Inner>,
}

struct Inner {
    name: String,
    /// Credential key selected by the durable job; absent is the legacy source-keyed default.
    token_key: Option<String>,
    /// Only an explicitly selected, consented credential; never part of the component cache.
    credential: Option<crate::domain::Secret>,
    /// An env reference might itself name a normally inherited process variable such as PATH.
    selected_env: Option<String>,
    description: String,
    capabilities: SourceCapabilities,
    permissions: Permissions,
    /// Who has the access tokens of a source that signs in. Absent at install and in conformance unless explicitly
    /// supplied by the synthetic fixture provider; no missing provider ever falls back to ambient credentials.
    tokens: Option<Arc<dyn AccessTokens>>,
    engine: &'static Engine,
    pre: SourcePre<HostState>,
    raw_pre: wasmtime::component::InstancePre<HostState>,
    reconciliation: Option<reconciliation::Extension>,
    memory_bytes: usize,
    timeout: Duration,
    /// Asked of the component once at load: it is constant, and the contract returns `&str`.
    default_room: String,
}

impl std::fmt::Debug for WasmAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmAdapter")
            .field("name", &self.inner.name)
            .finish_non_exhaustive()
    }
}

impl WasmAdapter {
    /// Load the component `bytes` as the source `manifest` describes.
    ///
    /// Compiles (cached by digest), links the host functions, and asks the component for its default room, which
    /// proves it implements the contract before a job depends on it. Blocking: call from a blocking context.
    ///
    /// # Errors
    ///
    /// [`Error::SourceIncompatible`] when the manifest or the component does not fit this MemCastle, and
    /// [`Error::SourceFailed`] when the component does not run.
    pub fn load(manifest: &SourceManifest, bytes: &[u8], mining: &MiningConfig) -> Result<Self> {
        Self::load_with(manifest, bytes, mining, None)
    }

    /// As [`WasmAdapter::load`], with the daemon's access tokens for a source that signs in with OAuth
    /// (docs/adr/039).
    ///
    /// # Errors
    ///
    /// As [`WasmAdapter::load`].
    pub fn load_with(
        manifest: &SourceManifest,
        bytes: &[u8],
        mining: &MiningConfig,
        tokens: Option<Arc<dyn AccessTokens>>,
    ) -> Result<Self> {
        Self::load_with_key(manifest, bytes, mining, tokens, None)
    }

    /// Load using exactly the source configuration selected when the durable job was submitted.
    pub fn load_with_key(
        manifest: &SourceManifest,
        bytes: &[u8],
        mining: &MiningConfig,
        tokens: Option<Arc<dyn AccessTokens>>,
        configuration: Option<String>,
    ) -> Result<Self> {
        Self::load_with_credential(manifest, bytes, mining, tokens, configuration, None, None)
    }

    /// Load a job's consented source with its in-memory selected credential.
    pub fn load_with_credential(
        manifest: &SourceManifest,
        bytes: &[u8],
        mining: &MiningConfig,
        tokens: Option<Arc<dyn AccessTokens>>,
        configuration: Option<String>,
        credential: Option<crate::domain::Secret>,
        selected_env: Option<String>,
    ) -> Result<Self> {
        check_compatible(manifest)?;
        let name = manifest.source.name.clone();
        let incompatible = |reason: String| Error::SourceIncompatible {
            name: name.clone(),
            reason,
        };
        let engine = engine()?;
        let digest = sha256_hex(bytes);
        let component = match COMPONENTS.get(&digest) {
            Some(cached) => cached.clone(),
            None => {
                let compiled = Component::new(engine, bytes).map_err(|e| {
                    incompatible(format!("it is not a valid WebAssembly component: {e:#}"))
                })?;
                COMPONENTS.insert(digest, compiled.clone());
                compiled
            }
        };
        let mut linker = Linker::<HostState>::new(engine);
        wasmtime_wasi::p3::add_to_linker(&mut linker)
            .map_err(|e| incompatible(format!("{e:#}")))?;
        Source::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |state| state)
            .map_err(|e| incompatible(format!("{e:#}")))?;
        let raw_pre = linker
            .instantiate_pre(&component)
            .map_err(|error| incompatible(format!("cannot link source: {error:#}")))?;
        let reconciliation = reconciliation::Extension::load(
            &component,
            manifest.compatibility.reconciliation.as_deref(),
        )
        .map_err(incompatible)?;
        let pre = SourcePre::new(raw_pre.clone()).map_err(|e| {
            incompatible(format!(
                "it does not fit the `memcastle:source` contract this MemCastle implements: {e:#}"
            ))
        })?;

        // What the source asks for, never more than the host allows.
        let ceiling_mib = u64::from(mining.source_memory_mib);
        let memory_mib = manifest
            .limits
            .memory_mib
            .map_or(ceiling_mib, |asked| u64::from(asked).min(ceiling_mib));
        let timeout = Duration::from_secs(
            manifest
                .limits
                .timeout_secs
                .map_or(mining.source_timeout_secs, |asked| {
                    asked.min(mining.source_timeout_secs)
                }),
        );
        let mut inner = Inner {
            token_key: configuration,
            credential,
            selected_env,
            name,
            description: manifest.source.description.clone(),
            capabilities: manifest.capabilities,
            permissions: manifest.permissions.normalized(),
            tokens,
            engine,
            pre,
            raw_pre,
            reconciliation,
            memory_bytes: usize::try_from(memory_mib * 1024 * 1024).unwrap_or(usize::MAX),
            timeout,
            default_room: String::new(),
        };
        inner.default_room = block_on(inner.call(false, None, |store, guest| {
            Box::pin(async move { guest.call_default_room(store).await.map(Ok) })
        }))?;
        block_on(inner.validate_reconciliation())?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Run a blocking call from a synchronous trait method without stalling the async runtime's worker.
    fn blocking<R>(&self, work: impl FnOnce(&Inner) -> R) -> R {
        match tokio::runtime::Handle::try_current() {
            // Only multi-threaded Tokio can replace a worker while a synchronous trait method waits.
            // The bridge itself uses a separate executor, including for current-thread callers.
            Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| work(&self.inner))
            }
            _ => work(&self.inner),
        }
    }
}

impl Inner {
    /// One call into the component, in a sandbox built for it.
    ///
    /// `granted` is whether the manifest's permissions apply: `normalize` runs without them, so that it is pure
    /// whatever the source asked for. `f` returns the guest's own `result`, which is mapped to MemCastle's errors
    /// here, together with traps, timeouts and refused permissions.
    async fn call<R: Send>(
        &self,
        granted: bool,
        locator: Option<&str>,
        f: impl for<'a> FnOnce(
            &'a Accessor<HostState>,
            host::exports::memcastle::source::adapter::Guest,
        ) -> BoxFuture<'a, GuestResult<R>>
        + Send
        + 'static,
    ) -> Result<R> {
        let pre = self.pre.clone();
        self.invoke(granted, locator, move |store| {
            Box::pin(async move {
                let instance = pre.instantiate_async(&mut *store).await?;
                let guest = instance.memcastle_source_adapter().clone();
                store
                    .run_concurrent(async move |access| f(access, guest).await)
                    .await?
            })
        })
        .await
    }

    /// Optional interfaces share precisely the same sandbox, credential grants,
    /// time limits and failure mapping as the original six adapter functions.
    async fn invoke<R: Send>(
        &self,
        granted: bool,
        locator: Option<&str>,
        f: impl for<'a> FnOnce(&'a mut Store<HostState>) -> BoxFuture<'a, GuestResult<R>> + Send,
    ) -> Result<R> {
        let none = Permissions::default();
        let permissions = if granted { &self.permissions } else { &none };
        // A token is offered only to a call that is granted its permissions, and only for the sign-in the manifest
        // declares: `normalize` is pure, and a source that declared none has nothing to ask for.
        let grant = match (granted, &self.permissions.oauth, &self.tokens) {
            (true, Some(requirement), Some(provider)) => Some(TokenGrant {
                source: self.token_key.clone().unwrap_or_else(|| self.name.clone()),
                requirement: requirement.clone(),
                provider: Arc::clone(provider),
            }),
            _ => None,
        };
        let selected = if granted && self.permissions.credential {
            self.credential.clone()
        } else {
            None
        };
        let state = host::state(
            permissions,
            locator,
            self.memory_bytes,
            self.timeout,
            grant,
            CredentialSelection {
                value: selected,
                optional: granted
                    && self.permissions.credential
                    && !self.capabilities.needs_credentials,
            },
            self.selected_env.as_deref(),
        )
        .map_err(|message| self.failed(message))?;
        let deadline = state.deadline;
        let mut store = Store::new(self.engine, state);
        store.limiter(|state| &mut state.limits);
        // Direct async execution must also yield during CPU loops, or one source can starve its executor worker.
        store.set_epoch_deadline(1);
        store.epoch_deadline_async_yield_and_update(1);

        // Epochs bound CPU loops; a wall-clock deadline also bounds suspended WASI I/O.
        let outcome = tokio::time::timeout_at(deadline.into(), f(&mut store)).await;
        // The host import's future can be gone while its blocking worker still owns a native child.
        store.data_mut().stop_process().await;
        if outcome.is_err() || std::time::Instant::now() >= deadline {
            return Err(Error::SourceTimeout {
                name: self.name.clone(),
                secs: self.timeout.as_secs(),
            });
        }
        let outcome = outcome.expect("an elapsed call returned its timeout above");
        let denied = store.data().denied.clone();
        let credential = store.data_mut().credential_failure.take();
        let failure = match outcome {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => self.guest_error(error, denied),
            Err(trap) => self.trap_error(&trap, denied),
        };
        // The source asked for a token and was told why it has none; whatever it made of that, the fix is the
        // credential's, so that is what is reported.
        Err(credential.unwrap_or(failure))
    }

    fn failed(&self, message: impl Into<String>) -> Error {
        let mut message = message.into();
        if message.chars().count() > MAX_MESSAGE_CHARS {
            message = message.chars().take(MAX_MESSAGE_CHARS).collect::<String>() + "...";
        }
        Error::SourceFailed {
            name: self.name.clone(),
            message,
        }
    }

    fn guest_error(&self, error: wit::SourceError, denied: Option<String>) -> Error {
        match error {
            wit::SourceError::CursorInvalid(message) => Error::SourceCursorInvalid {
                adapter: self.name.clone(),
                message,
            },
            wit::SourceError::InvalidInput(message) => Error::invalid_input("locator", message),
            // A refused permission is the likely cause of whatever the source made of it, so it is what is reported.
            wit::SourceError::Failed(message) => match denied {
                Some(denied) => Error::SourcePermissionDenied {
                    name: self.name.clone(),
                    message: denied,
                },
                None => self.failed(message),
            },
        }
    }

    fn trap_error(&self, error: &wasmtime::Error, denied: Option<String>) -> Error {
        if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
            return Error::SourceTimeout {
                name: self.name.clone(),
                secs: self.timeout.as_secs(),
            };
        }
        if let Some(message) = denied {
            return Error::SourcePermissionDenied {
                name: self.name.clone(),
                message,
            };
        }
        self.failed(format!("{error:#}"))
    }

    /// Grants a source's directory permission can make use of: the canonical form of a locator, when the manifest
    /// asks for the `locator` directory, so that the guest and the preopen agree on the path.
    fn grant_locator(&self, locator: &str) -> String {
        if self
            .permissions
            .filesystem
            .read
            .iter()
            .any(|entry| entry == "locator")
        {
            std::fs::canonicalize(locator)
                .map_or_else(|_| locator.to_string(), |p| p.display().to_string())
        } else {
            locator.to_string()
        }
    }
}

fn to_wit(source: &SourceRef) -> wit::SourceRef {
    wit::SourceRef {
        source: source.source.clone(),
        account: source.account.clone(),
        locator: source.locator.clone(),
        options: source
            .options
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    }
}

fn from_wit(source: wit::SourceRef) -> SourceRef {
    SourceRef {
        source: source.source,
        account: source.account,
        locator: source.locator,
        options: source.options.into_iter().collect(),
    }
}

fn raw_to_wit(raw: &RawDocument) -> wit::RawDocument {
    wit::RawDocument {
        external_id: raw.external_id.clone(),
        revision: raw.revision.clone(),
        body: raw.body.clone(),
        metadata: raw.metadata.to_string(),
        occurred_at: raw.occurred_at.map(|at| at.to_rfc3339()),
    }
}

impl SourceAdapter for WasmAdapter {
    fn reconciles(&self) -> bool {
        self.inner.reconciliation.is_some()
    }

    async fn reconcile(
        &self,
        source: &SourceRef,
        known: &super::adapter::KnownDocument,
    ) -> Result<super::adapter::Reconciliation> {
        self.inner.reconcile_document(source, known).await
    }

    async fn acquire(
        &self,
        source: &SourceRef,
        candidate: &Candidate,
    ) -> Result<super::adapter::Acquisition> {
        if !self.reconciles() {
            return Ok(self.read(source, candidate).await?.map_or(
                super::adapter::Acquisition::Skip,
                super::adapter::Acquisition::Document,
            ));
        }
        self.inner.acquire_document(source, candidate).await
    }
    fn name(&self) -> &str {
        &self.inner.name
    }

    fn description(&self) -> &str {
        &self.inner.description
    }

    fn capabilities(&self) -> SourceCapabilities {
        self.inner.capabilities
    }

    fn identify(&self, locator: Option<&str>, options: &Options) -> Result<SourceRef> {
        self.blocking(|inner| {
            // The host hands over the canonical path of a directory it is about to grant, so that `identify` is the
            // one place two spellings of a place become one source, as for a built-in adapter.
            let locator = locator.map(|locator| inner.grant_locator(locator));
            let options: Vec<(String, String)> = options
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let argument = locator.clone();
            block_on(inner.call(true, locator.as_deref(), move |store, guest| {
                Box::pin(async move { guest.call_identify(store, argument, options).await })
            }))
            .map(from_wit)
        })
    }

    fn default_wing(&self, source: &SourceRef) -> String {
        self.blocking(|inner| {
            let fallback = || "unnamed".to_string();
            let source = to_wit(source);
            block_on(inner.call(false, None, move |store, guest| {
                Box::pin(async move { guest.call_default_wing(store, source).await.map(Ok) })
            }))
            // A wing name is cosmetic: a source that fails to give one files under a neutral name, and the
            // real failure surfaces from the calls that matter.
            .unwrap_or_else(|_| fallback())
        })
    }

    fn default_room(&self) -> &str {
        &self.inner.default_room
    }

    async fn discover(
        &self,
        source: &SourceRef,
        cursor: &Cursor,
        limit: usize,
    ) -> Result<Discovery> {
        if self.reconciles() {
            return self.inner.discover_reconciled(source, cursor, limit).await;
        }
        let inner = &self.inner;
        let argument = to_wit(source);
        let cursor = cursor.to_string();
        let limit = u32::try_from(limit).unwrap_or(u32::MAX);
        let found = inner
            .call(true, Some(&source.locator), move |store, guest| {
                Box::pin(async move { guest.call_discover(store, argument, cursor, limit).await })
            })
            .await?;
        let candidates = found
            .candidates
            .into_iter()
            .map(|candidate| {
                let cursor_after = serde_json::from_str(&candidate.cursor_after).map_err(|e| {
                    inner.failed(format!(
                        "candidate `{}` has a cursor-after that is not JSON: {e}",
                        candidate.external_id
                    ))
                })?;
                Ok(Candidate {
                    external_id: candidate.external_id,
                    cursor_after,
                    handle: candidate.handle,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Discovery {
            candidates,
            exhausted: found.exhausted,
            filtered: 0,
            cursor_after: None,
        })
    }

    async fn read(&self, source: &SourceRef, candidate: &Candidate) -> Result<Option<RawDocument>> {
        let inner = &self.inner;
        let argument = to_wit(source);
        let candidate = wit::Candidate {
            external_id: candidate.external_id.clone(),
            cursor_after: candidate.cursor_after.to_string(),
            handle: candidate.handle.clone(),
        };
        let read = inner
            .call(true, Some(&source.locator), move |store, guest| {
                Box::pin(async move { guest.call_read(store, argument, candidate).await })
            })
            .await?;
        read.map(|raw| {
            let metadata = serde_json::from_str(&raw.metadata).map_err(|e| {
                inner.failed(format!(
                    "document `{}` has metadata that is not JSON: {e}",
                    raw.external_id
                ))
            })?;
            let occurred_at = raw
                .occurred_at
                .as_deref()
                .map(|at| {
                    chrono::DateTime::parse_from_rfc3339(at)
                        .map(|at| at.with_timezone(&chrono::Utc))
                        .map_err(|e| {
                            inner.failed(format!(
                                "document `{}` has an occurred-at that is not RFC 3339: {e}",
                                raw.external_id
                            ))
                        })
                })
                .transpose()?;
            Ok(RawDocument {
                external_id: raw.external_id,
                revision: raw.revision,
                body: raw.body,
                metadata,
                occurred_at,
            })
        })
        .transpose()
    }

    fn normalize(&self, raw: &RawDocument) -> Result<CanonicalDocument> {
        self.blocking(|inner| {
            // No grants: normalization is pure by contract, and the host holds a source to it.
            let raw = raw_to_wit(raw);
            let canonical = block_on(inner.call(false, None, move |store, guest| {
                Box::pin(async move { guest.call_normalize(store, raw).await })
            }))?;
            Ok(CanonicalDocument {
                title: canonical.title,
                room: canonical.room,
                name: canonical.name,
                kind: match canonical.kind {
                    wit::SourceKind::File => SourceKind::File,
                    wit::SourceKind::Manual => SourceKind::Manual,
                    wit::SourceKind::Transcript => SourceKind::Transcript,
                    wit::SourceKind::Other => SourceKind::Other,
                },
                uri: canonical.uri,
                tags: canonical.tags,
                segments: canonical
                    .segments
                    .into_iter()
                    .map(|segment| Segment { text: segment.text })
                    .collect(),
            })
        })
    }
}

/// Synchronous adapter methods also work inside a current-thread Tokio runtime, where nested block_on would panic.
fn block_on<R: Send>(future: impl Future<Output = Result<R>> + Send) -> Result<R> {
    let failed = |message: String| Error::SourceFailed {
        name: "runtime".into(),
        message,
    };
    let run = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| failed(format!("cannot start the source executor: {error}")))?
            .block_on(future)
    };
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::scope(|scope| scope.spawn(run).join())
            .map_err(|_| failed("the source executor was interrupted".into()))?
    } else {
        run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synchronous_calls_drive_async_io_outside_and_inside_either_tokio_runtime() {
        let call = || {
            block_on(async {
                tokio::time::sleep(Duration::from_millis(1)).await;
                Ok(42)
            })
            .unwrap()
        };
        assert_eq!(call(), 42);
        for runtime in [
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
        ] {
            assert_eq!(runtime.block_on(async { call() }), 42);
        }
    }
}
