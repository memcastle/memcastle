# ADR-051: Embedding provider modules are independently selected Wasm capabilities

## Status

Accepted.

## Context

A plugin can distribute sources and agent integrations, but neither contract can produce query or drawer embeddings.
Named connections and profiles already define selection and provenance (ADR-050);
the plugin catalogue already carries an `embedding_provider` inventory (ADR-049).
An executable module needs its own compatibility contract and isolation without making an
OpenAI-compatible endpoint depend on an OpenAI product plugin.

## Decision

An `embedding_provider` module supplies `memcastle-embedding.toml` and a `wasm32-wasip2`
`embedding.wasm` component implementing `memcastle:embedding@0.1.0`.
Its own manifest declares the module ID, version, contract, supported model IDs and dimensions,
an optional restricted JSON schema for non-secret settings, and whether it needs network and
host-scoped credential access.
The parent plugin manifest names and hashes those files and agrees on the module's identity, schema and contract.
The daemon checks the component's declared models against its actual `models` export before making it selectable.
The component receives ordered text batches, selected model, dimension and validated settings;
the host verifies vector count, dimension and finite coordinates.

The Wasm host provides no files, environment, arguments or process execution.
Network is granted only when declared in the module manifest and expressly consented to during plugin install;
a credential is available only from the selected connection's `env` or `file` reference during an embedding call,
and only when that module declared access.
The permission digest is per embedding module, separate from source consents; a changed permission set requires new consent.
Credentials and their contents are excluded from module manifests, plugin records, job payloads and embedding spaces.

An `[embeddings.providers.NAME]` connection with `type = "plugin"` selects an installed plugin ID and embedding module ID.
Its ordinary `[embeddings.profiles.NAME]` supplies the model and dimensions, so default, miner override,
explicit job and search selection share the same precedence and validation as Core providers.
The daemon starts with an unavailable plugin-backed profile;
attempting to select it reports the missing installation while other profiles continue to work.
Successful installation refreshes the scheduler's and search's common provider registry without a restart.
Uninstall is refused while a profile or active pinned job references that plugin;
updates removing referenced modules or replacing bytes used by an active pinned job are refused.

A plugin-backed embedding space fingerprints the non-secret connection settings, plugin and module identities,
module version and component digest along with ADR-050's model, dimension and pipeline identity.
A changed implementation therefore gets a new space rather than reusing old vectors; a resumed job refuses to cross spaces.
Core's command and OpenAI-compatible HTTP implementations remain available without a plugin,
including OpenRouter and other compatible servers.

## Alternatives rejected

Treating a source miner or chat integration as an embedding provider was rejected because neither contract
advertises a vector model, its dimensions or a compatible embedding space.
Running plugin binaries directly was rejected in favour of the existing component isolation model:
an executable can inherit ambient files, environment and credentials without an install-time permission grant.
A second provider registry was rejected because the plugin archive is already the reviewed, versioned distribution unit.

## Consequences

Plugin installation and updates use the existing versioned plugin registry, checksums, ownership ledger and generation directories.
Providers can ship sources or integrations in the same archive without enabling those modules just to embed text.
An incompatible update requires finishing or canceling pinned jobs;
existing vectors remain in their original space until the operator requests re-embedding.
