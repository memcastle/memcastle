//! Resolve only the selected provider account, at the point where a request needs a token.

use std::io::Read;

use crate::config::provider_accounts::ProviderAccount;
use crate::domain::{CredentialRef, Secret};

/// The provider's secret, with neither its contents nor its location in an error.
pub fn resolve(account: &ProviderAccount) -> std::result::Result<Secret, &'static str> {
    let token = match account.auth.credential() {
        CredentialRef::Env { name } => std::env::var(name).map_err(|_| "credential unavailable")?,
        CredentialRef::File { path } => {
            let file = std::fs::File::open(path).map_err(|_| "credential unavailable")?;
            let mut contents = String::new();
            file.take(8 * 1024)
                .read_to_string(&mut contents)
                .map_err(|_| "credential unavailable")?;
            contents.lines().next().unwrap_or_default().to_owned()
        }
        CredentialRef::Oauth => return Err("unsupported provider authentication"),
    };
    if token.is_empty() || token.contains('\r') || token.contains('\n') {
        return Err("credential unavailable");
    }
    Ok(Secret::new(token))
}

/// A non-secret availability hint for the settings page; never reads a token into an API response.
#[must_use]
pub fn available(account: &ProviderAccount) -> bool {
    resolve(account).is_ok()
}
