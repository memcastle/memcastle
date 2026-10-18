# ADR-050: Embedding spaces and provenance

## Status

Accepted.

## Context

An embedding is derived from canonical drawer content, an input pipeline, a model, and a provider connection.
Two 768-dimensional vectors from different models are no more comparable than vectors of different lengths.
The original `drawer.embedding` field and HNSW index record none of that information.

## Decision

Named embedding profiles select a named provider connection, model, and dimension.
A non-secret fingerprint includes the connection identity, provider protocol, model, dimension, input truncation length,
and pipeline version; credential material is never stored in jobs or vectors.
One derived `drawer_embedding` record per drawer and fingerprint holds the vector, the original content hash, the hash
of the actual provider input, and the full non-secret provenance.
The selected profile and fingerprint are pinned in durable jobs.
On resume, a missing or changed connection fails the job instead of silently changing its vector space.
Automatic jobs fill only missing vectors; explicitly requested `stale` and `all` refreshes may replace derived records.

The old field's vectors are retained as unknown provenance, not assigned to a default profile or read by profile search.
SurrealKit's version-4 destructive rollout retires the old HNSW index while keeping its field's values.
The `embedding-quarantine` data-migration watermark records that transition; re-embedding is a separately requested,
resumable job and never happens during database migration.
Canonical content, graph links and temporal validity do not depend on derived vectors.

SurrealDB HNSW indexes have fixed dimensions and an index cannot contain two incompatible spaces merely because they
have the same dimension.
The profile-filtered exact-search path works for arbitrary positive dimensions, applying the selected fingerprint and
drawer scope inside the database before ranking or limiting.
Schema-declared 768/1536/3072 projection tables provide dedicated index tiers; a tier can only be assigned to one
immutable space at a time.
Other spaces of the same dimension use exact ranking instead of sharing its HNSW index.
No vector from a provenance-free drawer is eligible for any profile-aware semantic result.

## Alternatives rejected

Reusing the drawer's single HNSW index for every 768-dimensional model was rejected because length alone cannot establish
compatibility, and a dimension-filtered search would still leave incompatible vectors in the same physical index.
Creating arbitrary indexes at runtime was rejected because MemCastle delegates all schema definitions to SurrealKit;
dynamic profile names and arbitrary vector lengths cannot safely generate schema DDL in application code.
Discarding all legacy vectors during migration was rejected because their contents may still be useful for manual recovery.

## Consequences

Changing a profile's model or endpoint creates a new space, so old vectors remain recoverable but cannot be searched
under the new configuration until an explicit re-embedding job populates it.
Exact ranking is slower for large palaces than a dedicated HNSW index, but never compares incompatible vectors.
Caller-supplied vectors need an explicit configured profile, including when the connection is `external` and the
daemon has no embedding provider of its own.
