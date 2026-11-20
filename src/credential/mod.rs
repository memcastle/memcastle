//! OAuth credentials for mining sources (docs/adr/039).
//!
//! A source that cannot be reached with a static token declares `[permissions.oauth]`; this module is everything the
//! daemon does about it, so that no source implements its own sign-in, storage or renewal. It runs the sign-in
//! ([`Credentials::begin`] and [`Credentials::wait`]: the device flow, or the browser flow with PKCE), keeps the
//! tokens ([`store`]), and hands the mining runtime a fresh access token when it asks ([`AccessTokens`]), renewing it
//! first when it is about to expire.
//!
//! Nothing here touches the palace store, the jobs or the WebAssembly runtime: the runtime sees only the
//! [`AccessTokens`] trait, and only `app` and `server` construct or call a [`Credentials`]
//! (`tests/credential_isolation.rs`).

mod flow;
pub mod oauth;
pub mod store;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::config::CredentialsConfig;
use crate::domain::{AccessTokens, OAuthBinding, OAuthRequirement, Secret, SourceConfiguration};
use crate::error::{Error, Result};

pub use flow::{Challenge, FlowKind, FlowStatus};
use oauth::{OAuthError, TokenSet};
use store::{CredentialStore, StoredCredential};

/// An access token with less than this left is renewed before it is handed out, so that it does not expire in the
/// middle of the call that is about to use it.
const REFRESH_MARGIN: TimeDelta = TimeDelta::seconds(60);

/// How long an access token is assumed to last when the provider does not say (RFC 6749 makes `expires_in`
/// optional). Short, so that a wrong guess costs a refresh and not a failed call.
const ASSUMED_LIFETIME: TimeDelta = TimeDelta::seconds(300);

/// How fast a sign-in may poll a provider.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// The shortest time between two polls, whatever the provider asks for.
    pub min_poll: Duration,
    /// How much longer to wait after the provider says `slow_down` (RFC 8628 section 3.5 says five seconds).
    pub slow_down_step: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            min_poll: Duration::from_secs(1),
            slow_down_step: Duration::from_secs(5),
        }
    }
}

/// Where a source stands, for `source show` and `source auth`. Carries no token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignInState {
    /// Whether there is a credential that would be used.
    pub signed_in: bool,
    /// When the access token last seen stops being valid, if known. A refresh token renews it, so this is not a
    /// deadline for the source.
    pub expires_at: Option<DateTime<Utc>>,
    /// What was granted.
    pub scopes: Vec<String>,
}

/// The end of a successful sign-in. Carries no token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIn {
    /// The source.
    pub source: String,
    /// Always true; present so the answer reads the same as [`SignInState`].
    pub signed_in: bool,
    /// When the access token stops being valid, if known.
    pub expires_at: Option<DateTime<Utc>>,
    /// What was granted.
    pub scopes: Vec<String>,
    /// Where the credential is kept now: `keyring` or `file`.
    pub stored_in: String,
}

struct Cached {
    token: Secret,
    expires_at: DateTime<Utc>,
    fingerprint: String,
    binding: OAuthBinding,
}

struct Inner {
    store: Box<dyn CredentialStore>,
    /// Access tokens by source. Memory only: they are short-lived and cheap to obtain again from the refresh token.
    cache: Mutex<HashMap<String, Cached>>,
    /// One lock per source, held across a refresh, so that many callers share one renewal. A provider that rotates
    /// refresh tokens would otherwise see the old one used twice and revoke the whole grant.
    refreshing: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    flows: Mutex<flow::Flows>,
    timing: Timing,
    configuration_reader: Mutex<Option<ConfigurationReader>>,
    /// Reserved before provider I/O, so cancelled startup/completion cannot resurrect a grant.
    generations: Mutex<HashMap<String, String>>,
}

type ConfigurationReader = Arc<dyn Fn() -> Result<Vec<SourceConfiguration>> + Send + Sync>;

/// The daemon's OAuth credentials: sign-in, storage and renewal for every source that needs them.
#[derive(Clone)]
pub struct Credentials {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials").finish_non_exhaustive()
    }
}

/// Lock a mutex, whatever a panicking holder left: nothing under these locks is left half-updated.
fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Credentials {
    /// Credentials kept in `store`.
    #[must_use]
    pub fn new(store: Box<dyn CredentialStore>) -> Self {
        Self::with_timing(store, Timing::default())
    }

    /// As [`Credentials::new`], polling a provider at the given pace.
    #[must_use]
    pub fn with_timing(store: Box<dyn CredentialStore>, timing: Timing) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                cache: Mutex::default(),
                refreshing: Mutex::default(),
                flows: Mutex::default(),
                timing,
                configuration_reader: Mutex::default(),
                generations: Mutex::default(),
            }),
        }
    }

    /// Credentials kept where the configuration says.
    #[must_use]
    pub fn from_config(config: &CredentialsConfig) -> Self {
        Self::new(store::from_config(config))
    }

    /// The application owns configuration I/O; token calls recheck it even during a long-running job.
    pub fn with_configuration_reader(&self, reader: ConfigurationReader) {
        *locked(&self.inner.configuration_reader) = Some(reader);
    }

    fn is_explicit_configuration(&self, key: &str) -> Result<bool> {
        let reader = locked(&self.inner.configuration_reader).clone();
        reader.map_or(Ok(false), |read| {
            Ok(read()?.iter().any(|config| config.id == key))
        })
    }

    fn configuration_matches(&self, binding: &OAuthBinding) -> Result<bool> {
        let reader = locked(&self.inner.configuration_reader).clone();
        let Some(reader) = reader else {
            return Ok(true);
        };
        let configs = reader()?;
        Ok(
            match configs
                .iter()
                .find(|config| config.id == binding.configuration)
            {
                Some(config) => {
                    config.source == binding.source
                        && config
                            .oauth_client_id
                            .as_deref()
                            .unwrap_or(&binding.declared.client_id)
                            == binding.client_id
                }
                None => {
                    !binding.explicit_configuration
                        && binding.configuration == binding.source
                        && binding.client_id == binding.declared.client_id
                }
            },
        )
    }

    /// Remove only a slot whose *stored* authority is obsolete, never a newer grant an old job asks for.
    fn current(&self, source: &str) -> Result<Option<StoredCredential>> {
        let stored = self.inner.store.load(source)?;
        if let Some(mut stored) = stored {
            if let Some(binding) = &mut stored.binding
                && binding.configuration == source
                && binding.grant_id.is_some()
                && self.configuration_matches(binding)?
            {
                if !binding.explicit_configuration && self.is_explicit_configuration(source)? {
                    // Saving an implicit default does not require another sign-in, but its later removal must revoke it.
                    binding.explicit_configuration = true;
                    self.inner.store.save(source, &stored)?;
                }
                return Ok(Some(stored));
            }
            self.forget_locked(source)?;
        }
        Ok(None)
    }

    /// Snapshot a slot for a newly submitted job; no grant means deliberately offline-only.
    pub fn binding(&self, mut binding: OAuthBinding) -> Result<OAuthBinding> {
        binding.grant_id = None;
        binding.explicit_configuration = self.is_explicit_configuration(&binding.configuration)?;
        let guard = self.lock_for(&binding.configuration);
        let _held = locked(&guard);
        let stored = self.current(&binding.configuration)?;
        if !self.configuration_matches(&binding)? {
            return Err(binding.required());
        }
        if let Some(stored) = stored
            && let Some(saved) = stored.binding
            && saved.source == binding.source
            && stored.fingerprint == binding.requirement().fingerprint()
        {
            binding.grant_id = saved.grant_id;
        }
        Ok(binding)
    }

    fn lock_for(&self, source: &str) -> Arc<Mutex<()>> {
        Arc::clone(
            locked(&self.inner.refreshing)
                .entry(source.to_string())
                .or_default(),
        )
    }

    fn cached(&self, source: &str, fingerprint: &str, binding: &OAuthBinding) -> Option<Secret> {
        let cache = locked(&self.inner.cache);
        let entry = cache.get(source)?;
        (entry.fingerprint == fingerprint
            && entry.binding.same_authority(binding)
            && entry.expires_at - Utc::now() > REFRESH_MARGIN)
            .then(|| entry.token.clone())
    }

    fn remember(
        &self,
        source: &str,
        fingerprint: &str,
        token: &Secret,
        expires_at: DateTime<Utc>,
        binding: &OAuthBinding,
    ) {
        locked(&self.inner.cache).insert(
            source.to_string(),
            Cached {
                token: token.clone(),
                expires_at,
                fingerprint: fingerprint.to_string(),
                binding: binding.clone(),
            },
        );
    }

    /// Keep what a sign-in or a renewal returned: the credential in the store, the access token in memory.
    ///
    /// `previous_refresh` is what a renewal that rotated nothing keeps using.
    fn install(
        &self,
        source: &str,
        requirement: &OAuthRequirement,
        tokens: &TokenSet,
        previous_refresh: Option<&str>,
        save: bool,
        binding: &OAuthBinding,
    ) -> Result<(DateTime<Utc>, &'static str, Vec<String>)> {
        let now = Utc::now();
        let expires_at = now
            + tokens
                .expires_in
                .and_then(|secs| i64::try_from(secs).ok())
                .map_or(ASSUMED_LIFETIME, TimeDelta::seconds);
        let refresh_token = tokens
            .refresh_token
            .as_ref()
            .map(|token| token.expose().to_string())
            .or_else(|| previous_refresh.map(str::to_string));
        let scopes = if tokens.scopes.is_empty() {
            requirement.scopes.clone()
        } else {
            tokens.scopes.clone()
        };
        let fingerprint = requirement.fingerprint();
        let mut stored_in = "memory";
        if save {
            let credential = StoredCredential {
                binding: Some(binding.clone()),
                fingerprint: fingerprint.clone(),
                // With no refresh token the access token is all there is to keep, and it is kept until it expires.
                access_token: refresh_token
                    .is_none()
                    .then(|| tokens.access_token.expose().to_string()),
                expires_at: refresh_token.is_none().then_some(expires_at),
                refresh_token,
                scopes: scopes.clone(),
                obtained_at: now,
            };
            stored_in = self.inner.store.save(source, &credential)?;
        }
        self.remember(
            source,
            &fingerprint,
            &tokens.access_token,
            expires_at,
            binding,
        );
        Ok((expires_at, stored_in, scopes))
    }

    /// Where `source` stands, without asking the provider for anything.
    ///
    /// # Errors
    ///
    /// [`Error::CredentialStoreFailed`] when the store cannot be read.
    pub fn state(&self, source: &str, requirement: &OAuthRequirement) -> Result<SignInState> {
        let guard = self.lock_for(source);
        let _held = locked(&guard);
        let signed_out = SignInState {
            signed_in: false,
            expires_at: None,
            scopes: Vec::new(),
        };
        let Some(stored) = self.current(source)? else {
            return Ok(signed_out);
        };
        if stored.fingerprint != requirement.fingerprint() {
            return Ok(signed_out);
        }
        let usable = stored.refresh_token.is_some()
            || stored
                .expires_at
                .is_some_and(|at| at - Utc::now() > REFRESH_MARGIN);
        let cached = locked(&self.inner.cache)
            .get(source)
            .filter(|entry| {
                entry.fingerprint == stored.fingerprint
                    && stored
                        .binding
                        .as_ref()
                        .is_some_and(|binding| binding.same_authority(&entry.binding))
            })
            .map(|entry| entry.expires_at);
        Ok(SignInState {
            signed_in: usable,
            expires_at: cached.or(stored.expires_at),
            scopes: stored.scopes,
        })
    }

    /// Forget `source`'s credential, as removing the source does.
    ///
    /// # Errors
    ///
    /// [`Error::CredentialStoreFailed`] when the store cannot be written.
    pub fn forget(&self, source: &str) -> Result<()> {
        let guard = self.lock_for(source);
        let _held = locked(&guard);
        self.forget_locked(source)
    }

    fn forget_locked(&self, source: &str) -> Result<()> {
        locked(&self.inner.cache).remove(source);
        locked(&self.inner.generations).remove(source);
        self.cancel_flow(source);
        self.inner.store.delete(source)
    }

    /// Renew `source`'s access token. Holds the source's lock, so a caller that arrives meanwhile waits for this
    /// renewal and then finds its result in the cache.
    fn renew(
        &self,
        source: &str,
        requirement: &OAuthRequirement,
        expected: Option<&OAuthBinding>,
    ) -> Result<Secret> {
        let fingerprint = requirement.fingerprint();
        let guard = self.lock_for(source);
        let _held = locked(&guard);
        let required = |reason: &str| Error::CredentialRequired {
            source_name: source.to_string(),
            reason: reason.to_string(),
        };
        let stored = self
            .current(source)?
            .ok_or_else(|| required("it has never been signed in"))?;
        let binding = stored
            .binding
            .as_ref()
            .ok_or_else(|| required("sign in again to bind this legacy credential"))?;
        if let Some(expected) = expected
            && !self.configuration_matches(expected)?
        {
            return Err(expected.required());
        }
        if expected.is_some_and(|expected| !expected.same_authority(binding)) {
            return Err(expected.expect("checked above").required());
        }
        if stored.fingerprint != fingerprint {
            return Err(required(
                "it was signed in under other terms than the installed source asks for now (client, endpoints or scopes)",
            ));
        }
        // Store and grant validation precede cache lookup, so a warm cache cannot borrow a replacement sign-in.
        if let Some(token) = self.cached(source, &fingerprint, binding) {
            return Ok(token);
        }
        let Some(refresh_token) = stored.refresh_token.clone() else {
            // No refresh token: the access token is all there was, and it is used until it expires.
            return match (&stored.access_token, stored.expires_at) {
                (Some(token), Some(at)) if at - Utc::now() > REFRESH_MARGIN => {
                    let token = Secret::new(token.clone());
                    self.remember(source, &fingerprint, &token, at, binding);
                    Ok(token)
                }
                _ => Err(required(
                    "its access token expired and the provider issued no refresh token to renew it with",
                )),
            };
        };
        let refreshed = refresh_blocking(requirement.clone(), refresh_token.clone());
        // Provider I/O can overlap a file edit or another backend user's sign-in; obsolete results carry no authority.
        let current_binding = self
            .current(source)?
            .and_then(|stored| stored.binding)
            .ok_or_else(|| binding.required())?;
        if !current_binding.same_authority(binding) {
            return Err(binding.required());
        }
        // Provider I/O may have promoted an implicit default; rotation must keep that newly observed provenance.
        let binding = &current_binding;
        match refreshed {
            Ok(tokens) => {
                let rotated = tokens
                    .refresh_token
                    .as_ref()
                    .is_some_and(|new| new.expose() != refresh_token);
                // The store is written only when there is something new to keep: most renewals rotate nothing.
                self.install(
                    source,
                    requirement,
                    &tokens,
                    Some(&refresh_token),
                    rotated,
                    binding,
                )?;
                tracing::debug!(source, rotated, "renewed an access token");
                Ok(tokens.access_token)
            }
            Err(OAuthError::Revoked(message)) => {
                // The provider will never honour this credential again, so keeping it only makes every later run fail
                // the same way and hides that the fix is to sign in.
                locked(&self.inner.cache).remove(source);
                self.inner.store.delete(source)?;
                tracing::info!(
                    source,
                    "the provider revoked a stored credential; it was removed"
                );
                Err(Error::CredentialRequired {
                    source_name: source.to_string(),
                    reason: format!("the provider no longer honours it ({message})"),
                })
            }
            Err(OAuthError::Rejected(message) | OAuthError::Unreachable(message)) => {
                Err(Error::CredentialRefreshFailed {
                    source_name: source.to_string(),
                    message,
                })
            }
        }
    }
}

impl AccessTokens for Credentials {
    fn validate_binding(&self, binding: &OAuthBinding) -> Result<()> {
        let current = self.binding(OAuthBinding {
            grant_id: None,
            ..binding.clone()
        })?;
        if binding.grant_id.is_none()
            || !current.same_authority(binding)
            || !self.configuration_matches(binding)?
            || !self
                .state(&binding.configuration, &binding.requirement())?
                .signed_in
        {
            return Err(binding.required());
        }
        Ok(())
    }

    fn bound_access_token(&self, binding: &OAuthBinding) -> Result<Secret> {
        if binding.grant_id.is_none() {
            return Err(binding.required());
        }
        self.renew(
            &binding.configuration,
            &binding.requirement(),
            Some(binding),
        )
        .map_err(|error| match error {
            Error::CredentialRequired { reason, .. } => binding.unavailable(reason),
            other => other,
        })
    }

    fn access_token(&self, source: &str, requirement: &OAuthRequirement) -> Result<Secret> {
        self.renew(source, requirement, None)
    }

    fn is_signed_in(&self, source: &str, requirement: &OAuthRequirement) -> bool {
        self.state(source, requirement)
            .is_ok_and(|state| state.signed_in)
    }
}

/// Renew on a thread and runtime of its own.
///
/// The caller is a synchronous host call from inside a WebAssembly source, which may be on a runtime worker or not, and
/// `block_on` from a runtime worker panics. A thread and a current-thread runtime work from anywhere, and a renewal
/// is rare enough (about once an hour per source) that starting them is free.
fn refresh_blocking(
    requirement: OAuthRequirement,
    refresh_token: String,
) -> std::result::Result<TokenSet, OAuthError> {
    let unreachable = |message: String| OAuthError::Unreachable(message);
    std::thread::Builder::new()
        .name("memcastle-credential-refresh".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| OAuthError::Unreachable(format!("cannot start a runtime: {e}")))?;
            runtime.block_on(async {
                // A client of its own: one made on another runtime keeps connections that this one cannot drive.
                let client = oauth::client()?;
                oauth::refresh(&client, &requirement, &refresh_token).await
            })
        })
        .map_err(|e| unreachable(format!("cannot start a thread: {e}")))?
        .join()
        .unwrap_or_else(|_| Err(unreachable("the renewal panicked".to_string())))
}

#[cfg(test)]
pub(crate) mod fake;

#[cfg(test)]
mod tests;
