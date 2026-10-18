# ADR-052: Provider accounts are independent of capability connections

## Status

Accepted.

## Context

An OpenAI or OpenRouter account can serve more than one capability and more than one endpoint.
Embedding connections currently hold their own credentials, while source OAuth sign-ins are scoped to source installations.
Sharing a source session or copying keys into each embedding profile would conflate distinct lifecycles.

## Decision

Core defines named `[provider_accounts.ID]` entries with a provider identity and a generic authentication mechanism.
An account references an existing daemon environment variable or owner-controlled file, never an inline secret.
Core supports `bearer` and `api_key` authentication; the latter has an optional public HTTP header name.
An embedding connection selects an account by ID and declares the same provider identity separately from its protocol,
endpoint, module and settings.
Several connections can select the same account, while several accounts for one provider remain distinct.
Future LLM connections can use the same account contract and resolver without introducing an LLM subsystem here.

Credentials are resolved by the daemon at use time, allowing rotation without restarting the server.
The HTTP provider sends a token in the selected header.
The Wasm host binds only the selected account to one embedding component and serves it only during an embedding call,
when that module declared credential access and installation was consented to.
The host never supplies the account registry, daemon environment, filesystem or a token during module discovery.
The settings API reports account IDs, provider IDs, auth kinds, availability and embedding connection linkage,
but never a credential location or value.

Source-specific OAuth grants remain tied to their source and manifest permissions.
Product-specific interactive or web-session sign-ins require a separate, explicit provider/integration authentication
implementation that defines its renewal and capability contracts; a chat sign-in never implies embedding access.
The existing source credential store is not a generic, named provider-token store.
Old inline embedding keys/references and legacy single-provider settings continue to work; accounts are opt-in.

Account IDs, credential references, tokens and availability never contribute to embedding-space provenance.
Legacy HTTP URLs containing userinfo are fingerprinted without their user/password, so rotating them cannot change a space.
Endpoint, provider identity/protocol, non-secret plugin configuration and version, model and dimensions still distinguish
vector spaces under ADR-050 and ADR-051.
Removing the old plugin credential-reference hash changes the fingerprint of existing plugin profiles once;
their previous vectors remain in their old space and need deliberate re-embedding.

## Consequences

Editing provider accounts requires editing the daemon configuration and restarting to change the selected account or
credential reference.
The secret behind an existing reference may be rotated without restarting.
Missing credentials fail the selected request with a redacted, actionable error; unrelated accounts and profiles keep working.
