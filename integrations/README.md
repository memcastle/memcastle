# Integrations

An **integration** is lifecycle/platform glue for one agent ecosystem — it
decides *when* to call MemCastle (session start, before context compaction, on
an interval, on a manual command), never *how* memory works. All memory
semantics — checkpoint persistence, wake-up composition, recall/search,
knowledge-graph mutation, mining — live in the MemCastle daemon (`src/`), behind
`app::AppServices`, and are reused identically by every integration.

See [`docs/architecture.md`](../docs/architecture.md) and `PLAN.md` for the full
rationale.

Every integration satisfies the same conformance matrix, defined in
[`docs/integration-contract.md`](../docs/integration-contract.md): what MemCastle
operation each capability uses, what the client is responsible for, and what to
test. The daemon half is replayed from the language-neutral fixtures in
`tests/fixtures/integration/`; the client half is each integration's own tests.
Where a client has no lifecycle point for a capability, say so in that
integration's README (missing, fallback, effect) instead of faking one.

## Hard rule

An integration talks to MemCastle **exclusively** over its HTTP API and MCP
tools. It must never:

- reimplement memory logic (checkpoint classification into destinations is the
  one exception — see below — everything else is a MemCastle call),
- touch `store`/`jobs` directly, or embed a second copy of the persistence
  layer,
- fork the memory model per ecosystem.

The `integrations-http-only` hook enforces this: it fails on `surrealdb`,
`surrealkv`, `SurrealStore`, a `store`/`jobs` path or `/api/db` anywhere under
this directory (Markdown and `node_modules` excepted).

The one deliberate exception: **deciding what's worth remembering** (which
words in a conversation become a checkpoint item, and which destination —
preference / project / diary / general, plus an optional fact mutation — each
belongs to) requires the
integration's own model and full conversation context, which MemCastle does not
have. MemCastle persists whatever already-classified payload it's given; it
does not classify.

## Layout

Core ships no agent-specific integration assets.
The Pi, OpenCode, Claude Code and Codex integrations and their client tests ship from the standalone
[`memcastle-pi`](https://github.com/memcastle/memcastle-pi),
[`memcastle-opencode`](https://github.com/memcastle/memcastle-opencode),
[`memcastle-anthropic`](https://github.com/memcastle/memcastle-anthropic) and
[`memcastle-openai`](https://github.com/memcastle/memcastle-openai) repositories, respectively.

Each ecosystem uses its own native language/runtime and packaging conventions —
this is not forced into Rust, and there is no shared plugin runtime or package
manager across them. Each plugin releases independently of Core and the other providers.

## Install and distribution

Users install an integration with `memcastle integration install <agent>`; see
[Agent integrations](../docs/integrations.md).
Each external integration carries a `memcastle-integration.toml` (id, version, supported versions and files to install).
Core retains the local installer and shared skills, but packages no agent-specific integration assets.
Install a provider plugin before asking `memcastle integration install` to register its agent module
([ADR-034](../docs/adr/034-agent-integration-distribution.md)).

## Skills

Shared instructions live in [`skills/`](../skills/README.md) and can be installed on their own
(see [Agent skills](../docs/skills.md)).
An integration's `memcastle-integration.toml` names the skills it exposes, a `[[skills]]` entry each: the shared ones by
name, and one that only this integration needs as `local = true` from its own `skills/` directory.
`memcastle integration install` copies them into the installed copy, so no separate skill installation follows.
The provider integration owns the lifecycle: when to load a skill, when to call MemCastle, and loading none into a
session whose memory mode is off.

## Per-ecosystem findings

- [`memcastle-opencode` research](https://github.com/memcastle/memcastle-opencode/blob/main/modules/opencode/docs/research.md)
  maps OpenCode's plugin hooks, MCP client and skills onto
  MemCastle operations, and records where OpenCode has no clean equivalent.
- [`memcastle-anthropic` research](https://github.com/memcastle/memcastle-anthropic/blob/main/modules/claude-code/docs/research.md) records Claude Code's supported declarative plugin API
  and the lifecycle gaps that it cannot safely emulate.

## Non-goals

No plugin marketplace, no dynamic plugin runtime, no package manager across
ecosystems — this is plain directories with markdown and each ecosystem's own
native tooling, nothing more. `memcastle integration` copies a bundle and uses the agent's own command to register it;
it is not a package manager and resolves nothing.

## Status

The external OpenCode package tests connection, mode, discovery and failure handling against a real daemon.
It supports wake-up at session start, search-before-answer, interval/manual/emergency checkpoints, project context and
manual audit/repair.
Background mining is daemon-side: a user-enabled [trigger](../docs/triggers.md) schedules a miner without an agent timer.
In `read-only` mode the client sends no write; in `off` it registers nothing.
Its persistent MCP connection is re-established with the same session mode if the daemon forgets it.
The Pi extension has its own client tests in [`memcastle-pi`](https://github.com/memcastle/memcastle-pi).
Tracked as GitHub issues under the
"Phase 2 — Pi integration", "Phase 3 — OpenCode integration", and
"Phase 4 — Claude Code integration" milestones.
The independently released Claude Code package uses its local marketplace with native HTTP MCP, shared skills, prompt
hooks, and explicit commands; its README documents the unsupported automatic lifecycle work as gaps.
