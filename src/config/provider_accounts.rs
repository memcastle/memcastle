//! Provider identity and authentication shared by embedding and future capability connections.

use serde::{Deserialize, Serialize};

use crate::domain::CredentialRef;
use crate::error::{Error, Result};

/// A named account at a provider, independent of protocol, endpoint and model.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderAccount {
    /// Service identity, such as `openai` or `openrouter`.
    pub provider_id: String,
    /// The authentication mechanism understood by Core.
    pub auth: ProviderAuth,
}

/// Generic authentication; interactive sessions require a separate, explicit integration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderAuth {
    /// An `Authorization: Bearer` token.
    Bearer {
        /// Read only by the daemon at the time of use.
        #[serde(skip_serializing)]
        credential: CredentialRef,
    },
    /// A token sent in a named HTTP header (default `x-api-key`).
    ApiKey {
        /// Read only by the daemon at the time of use.
        #[serde(skip_serializing)]
        credential: CredentialRef,
        /// Public header name, never a header value.
        #[serde(default)]
        header: Option<String>,
    },
}

impl ProviderAuth {
    /// The only credential selected for this account.
    #[must_use]
    pub fn credential(&self) -> &CredentialRef {
        match self {
            Self::Bearer { credential } | Self::ApiKey { credential, .. } => credential,
        }
    }

    /// The non-secret authentication mechanism for inspection.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bearer { .. } => "bearer",
            Self::ApiKey { .. } => "api_key",
        }
    }

    /// Validate before a provider can use an account; never print a credential location.
    pub fn validate(&self, account: &str) -> Result<()> {
        match self.credential() {
            CredentialRef::Env { name }
                if !name.is_empty()
                    && name
                        .bytes()
                        .next()
                        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                    && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') => {}
            CredentialRef::File { path } if std::path::Path::new(path).is_absolute() => {}
            _ => {
                return Err(Error::config(format!(
                    "provider account `{account}` requires an env name or absolute file credential reference; source OAuth is not a provider account"
                )));
            }
        }
        if let Self::ApiKey {
            header: Some(header),
            ..
        } = self
            && (reqwest::header::HeaderName::from_bytes(header.as_bytes()).is_err()
                || header.eq_ignore_ascii_case("authorization")
                || header.eq_ignore_ascii_case("host"))
        {
            return Err(Error::config(format!(
                "provider account `{account}` needs a valid API-key header other than Authorization or Host; use bearer authentication for Authorization"
            )));
        }
        Ok(())
    }
}
