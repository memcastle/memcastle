# ADR-053: Core owns source reconciliation inventory and serializes mining writers

## Status

Accepted.

## Context

API-backed sources need to recheck previously mined IDs when objects disappear, lose sharing or leave a miner's scope.
The original contract exposes only discovery candidates and optional raw documents.
An empty discovery cannot checkpoint progress, and a skipped read does not mean deletion.
Keeping inventory in a provider cursor makes `--full` discard correctness state and makes the provider implement its own
write-ahead registration to cover interrupted first ingestion.
Job leases prevent two workers claiming one job but do not prevent different jobs overwriting the same source cursor.

## Decision

Keep the original `memcastle:source@0.4.0` world binary-compatible.
An explicit `compatibility.reconciliation = "0.1.0"` selects an additional exported interface and the
`reconciled-source` world.
Older hosts reject the new compatibility key rather than silently losing reconciliation semantics.
The extension adds metadata-only inventory checks, explicit acquisition/retirement outcomes and page-level checkpoints.
No provider-specific code enters Core.

Core registers discovered IDs before acquisition and merges those registrations with existing source-document records.
Inventory and its bounded keyset-walk position live in the same SurrealDB palace, separately from the provider cursor.
Full scans and changed filters retain inventory; only confirmed provider decisions retire content.
Retirement closes all current origin chunks, including partial writes that preceded the first document record.
History remains queryable and a later acquisition can restore the same provider revision.
Inventory checks consume the document limit, and progress-only pages do not.

Named miners of reconciling sources are independently partitioned even when their provider reference and destination match.
Mutable filters are excluded from identity; a changed option set restarts the scan against the retained inventory.
Legacy identities remain unchanged, and activating reconciliation for an established provider requires a deliberate
transition for its older named-miner histories.
Renaming a reconciling miner creates another identity; stable names preserve state through filter edits.

All mining jobs acquire a source-level admission record before reading its cursor.
Admission follows the owning job's durable lease and can be reclaimed after completion, cancellation or expiry.
Canonical drawer, document, inventory and cursor mutations verify a unique ownership token in their own transactions.
Each mutation also writes the admission record, ensuring a takeover conflicts with an in-flight stale transaction under
snapshot isolation rather than merely checking an old snapshot of the token.
Release is conditional on the token, so an old worker cannot release its successor's lock.
Pause/cancel remains cooperative and different sources retain concurrency.

```mermaid
flowchart LR
    J[Mining job] --> L[Claim effective source]
    L --> I[Core inventory check]
    I --> D[Provider discovery]
    D --> R[Register ID before acquisition]
    R --> A[Acquire or explicitly retire]
    A --> W[Fenced canonical writes]
    W --> C[Commit source progress]
    C --> D
```

Offline conformance can preserve API locators and supply a fixed synthetic selected credential.
It never resolves real environment/file credentials or OAuth tokens.
The Wasm host receives only call arguments and existing consented grants, never database access.

## Consequences

Providers no longer need cursor inventories, synthetic progress documents or zero-segment tombstone conventions.
They still own API pagination, filtering, identity normalization and evidence for confirming removal.
An incomplete listing or a transient/global credential failure cannot implicitly retire documents.
Source-level serialization can make a second job wait for the first, and waiting jobs still occupy a scheduler slot.
Legacy sources keep their original acquisition and skip semantics.
