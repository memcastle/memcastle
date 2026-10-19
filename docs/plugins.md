# Plugin packages

This page describes the plugin manifest, archive, registry and module lifecycle contracts.

A **plugin** is a versioned, installable package published by one repository.
A **provider** names the shared upstream implementation family.
A plugin declares zero or more **modules**, each with a stable ID and a type: `source`, `integration` or `embedding_provider`.
A **source kind** is a mining capability exposed by a source module.
A configured **miner instance** selects one source and keeps its own scope and ingestion history.
An **integration instance** is separately installed into an agent from a plugin's integration module.

`docs/plugins.json` is the reviewed plugin discovery catalogue, published at `https://memcastle.github.io/plugins.json`.
It lists repository-level plugin IDs and module inventories; it is not a declaration that a package is safe.
The existing `registry.json` and `memcastle-index.json` remain the format-1 source catalogues for v0.4 consumers.
The new plugin catalogue must never be substituted for either legacy index.
Its format-1 `plugins[]` entries name an `id`, `description`, GitHub `owner/repository` and an inventory of
`{id, type}` modules.
Entries may list signed versions with `version`, archive `url` and `sha256`, or omit `versions` to discover
digest-bearing GitHub Release assets named `<plugin-id>-<version>.tar.gz`.
An entry may appear before its first release; it is searchable, but cannot be installed until an archive is published.
The source-only `memcastle-github` plugin is listed here rather than in the legacy source registry;
its `github` module must be installed with `memcastle plugin install github` before it can be enabled.
For a signed static entry, run `memcastle source keygen <private-key>` once, then
`memcastle plugin sign <archive> --key <private-key>` and publish its reported `sha256` and `signature` fields.
Users who require that publisher's signature set `plugins.trust = "required"` and list its public key under
`plugins.trusted_keys`.
An older version with a different module inventory may give its own `modules` list in its version entry.
The root inventory describes the newest release; a pinned older release without indexed module metadata is
identified and checked against the digest of its own archive.

The immutable root `plugin.toml` has `format = 1` and a `[plugin]` table containing `id`, `provider`,
`description`, `repository` and `license`.
It also declares a top-level `memcastle` version requirement, optional `[[dependencies]]` with `id`, `version`
and `optional`,
and any number of `[[modules]]`.
A required dependency must already be installed at a compatible version; MemCastle does not silently fetch another
plugin while installing this one.
An optional dependency may be absent, but is version-checked when present.
Removing a plugin blocks required dependents; optional dependents remain installed without it.
Each module specifies a stable `id`, `type`, paths `manifest` and `entry`,
and optionally `optional`, `config_schema`; source and embedding modules also require a versioned `contract`.
Source artifacts normally live under `sources/<id>/`, and integration artifacts under `integrations/<id>/`;
previously published packages using `modules/<id>/` remain installable. Both paths for one module must share that
module's selected directory. The typed roots let a source and integration share an ID without sharing files.
`plugin.toml` is not rewritten by packaging; `manifest.toml` holds the parent and module versions and checksums.
An optional top-level `artifacts = ["shared/config.json", "modules/example/support"]` lists additional project-relative
files or directories to include once for multiple modules; integration manifests continue to supply agent assets and
skills. Missing or symlinked declared artifacts are refused.
A source and an integration may share an ID (for example, Pi's two `pi` modules); identities are unique by `(type, id)`.
Two modules of the same type cannot share an ID, and their artifact paths must remain distinct.
The parent may also declare `shared_config_schema` and `[authentication]` with a public `kind` and a credential reference;
no login token or client secret is written into the manifest.
The OpenCode plugin likewise declares an `opencode` source and an `opencode` integration under one plugin.
The reviewed [`plugins.json`](plugins.json) catalogue lists `memcastle-pi`, `memcastle-opencode`,
`memcastle-anthropic` and `memcastle-openai`
repositories; until a provider publishes a release, build its archive locally before installing it.
Source modules retain their `memcastle-source.toml` and `source.wasm` contract;
integration modules retain their `memcastle-integration.toml` and agent assets.
Embedding modules use `embeddings/<id>/memcastle-embedding.toml` and `embeddings/<id>/embedding.wasm`
(the legacy `modules/<id>/` root is also accepted).
Their manifests declare a version, `contract = "0.1.0"`, one or more `[[models]]` with `id` and `dimensions`,
optional `network` and `credential` permissions, and an optional restricted `config_schema`.
For example, a plugin project can add this module to its root `plugin.toml`:

```toml
[[modules]]
id = "vendor-embeddings"
type = "embedding_provider"
manifest = "embeddings/vendor-embeddings/memcastle-embedding.toml"
entry = "embeddings/vendor-embeddings/embedding.wasm"
contract = "0.1.0"

[modules.config_schema]
type = "object"
required = ["region"]
additionalProperties = false

[modules.config_schema.properties.region]
type = "string"
```

The module's `memcastle-embedding.toml` then describes what that particular component supports:

```toml
id = "vendor-embeddings"
version = "0.1.0"
contract = "0.1.0"
network = true
credential = true

[[models]]
id = "vendor-embedding-v2"
dimensions = [768, 1536]

[config_schema]
type = "object"
required = ["region"]
additionalProperties = false

[config_schema.properties.region]
type = "string"
```

Compile the `memcastle:embedding@0.1.0` world in `wit/embedding/memcastle-embedding.wit` as a
`wasm32-wasip2` component before `plugin package`; the packager does not compile module code.
`models` must return the same models and dimensions as the manifest; `embed` receives a model, dimension,
JSON settings object and ordered text batch, returning one vector per input.
The host checks counts, lengths and finite coordinates, limits calls to 64 MiB and each connection's timeout,
and grants only the permissions the preview showed.
The supported settings schema accepts an object with typed scalar `properties`, `required` keys and
`additionalProperties = false`; credentials must be connection references rather than schema settings.
The parent module agrees on its contract, version and schema; the daemon verifies its Wasm component and advertised
models before installation and selects it through a named embedding connection, not a source or integration.
See [ADR-051](adr/051-embedding-provider-modules.md) and [embedding configuration](configuration.md#named-connections-and-profiles).

The tar.gz release archive contains generated `manifest.toml` and the declared module files, **not** `plugin.toml`.
The generated manifest combines the descriptor's metadata with release versions and SHA-256 for every other archive
file, including agent assets; it is required for installation, and earlier archives without it are not accepted.
`plugin package` also writes `dist/ID-VERSION.sha256sum` beside the archive for verifying the archive itself.
The archive is limited to 1024 canonical-path regular files, 64 MiB compressed and 256 MiB expanded;
symlinks, aliases, undeclared files and path escapes are refused before a generation is published.
Source and integration module identities and versions must match their own manifests.
The `memcastle-openai` plugin lists separate `chatgpt` and `codex` source IDs,
plus the `codex` agent integration: those source IDs remain distinct miner kinds with separate cursors.
Jira and Confluence likewise remain separate source modules even if one Atlassian plugin shares API-client code.
`shared_config_schema` and `[authentication]` describe provider-wide configuration and a public credential reference;
each module's own permissions and each miner's scope remain separate.
`memcastle plugin install` makes source modules available but leaves each new one installed and inactive.
`memcastle source enable/disable` controls its mining availability independently of other modules.
`memcastle integration install/remove` copies and registers an integration from an installed plugin's local files;
it never fetches a standalone module or needs a daemon to reach its agent.
Embedding providers become active only when a configured embedding profile selects their module.
The plugin preview asks separately for any embedding module's network or selected-credential permissions;
approve its `embedding:ID` digest with `--consent embedding:ID=DIGEST` or the interactive confirmation.
Installing or updating a plugin never enables a source or installs an agent integration on its behalf.
Plugin removal must preserve drawers, mined source records and cursors.
Module configuration, active pinned embedding jobs and dependent installations must be resolved before removal.
The daemon checks agent integration receipts in its own agents directory, not on other machines.
A copy installed on another machine and its agent configuration are not deleted by a plugin uninstall;
it can still use MCP/HTTP, but updating it from that plugin requires reinstalling the plugin first.
A plugin update cannot replace the bytes of an already installed version or downgrade a configured release;
publish a new plugin version, keeping stable module IDs and independent module versions.
A mined legacy source requires an explicit `--adopt-source <id>` when moving into a plugin.
After a Core release stops bundling a source, remove its old package row explicitly before installing a plugin
with that source ID; the mined data remains available for `--adopt-source`.
Uninstall keeps a source-ID ownership record so a different plugin cannot later inherit retained cursors or drawers.
The plugin record, source records and module-ID claims change together in one database transaction;
two plugins cannot claim the same live source, integration or embedding-provider ID after concurrent previews.
Historic source-ID claims remain after uninstall, while removed integrations can be provided by another plugin.
Before changing the ledger, the daemon parks the local integration pointer so it cannot serve old assets during the switch;
it restores that pointer if the transaction fails.
Only after the commit does the daemon publish the new pointer and remove retired source credentials.
If cleanup is interrupted, startup reconciles current pointers from the ledger, drops orphan pointers and retries
credential deletion for source IDs whose installation rows have gone.

GitTPL authoring templates are maintained separately as `memcastle-plugin.tpl`, `memcastle-source.tpl` and
`memcastle-integration.tpl`.
Plugin composition requires the released GitTPL 0.14 or newer; the Core checkout's older GitTPL tooling for its own
Rust project template is not the provider-template toolchain.
The parent composes source/integration templates for each selected module ID and records their resolved commits;
an empty initial plugin is valid, and another module can be added by changing the recorded answer and updating GitTPL.
Call `git tpl init` on those repositories directly rather than asking MemCastle to scaffold code.
GitTPL 0.14 currently refuses the same name in both composition lists; a provider with shared source/integration IDs
composes one module, overlays the other from its pinned child template, and records that step in its repository.

From a directory containing local clones of all three template repositories:

```sh
git tpl init "$PWD/memcastle-plugin.tpl" example --init \
  --answer plugin_id=example --answer 'sources=["files-one","files-two"]' \
  --answer 'integrations=["assistant"]' \
  --answer "source_template=$PWD/memcastle-source.tpl" \
  --answer "integration_template=$PWD/memcastle-integration.tpl" \
  --defaults --strict-answers --json
# Build every selected source and integration first, then package their existing artifacts.
memcastle plugin package example/plugin.toml --version 0.1.0
memcastle plugin install example/dist/example-0.1.0.tar.gz
```

This offline example records the machine-specific child paths in the generated project's configuration.
For a shared provider repository, use the published GitTPL and all three published template URLs from the start;
GitTPL records those URLs and the resolved child commits and does not rewrite local paths to remote URLs later.

Build source components with the provider's reproducible Cargo flags before calling Core's packager;
the packager only gathers validated, already-built artifacts.

With the three published `https://github.com/memcastle/*.tpl` URLs, `git tpl init` needs `--trust`
to allow the child-template clones.

To scaffold only a module, invoke `memcastle-source.tpl` or `memcastle-integration.tpl` directly.
For an existing provider project, edit its recorded `sources` or `integrations` array in
`.config/git.tpl.toml`, then run `git tpl update --defaults --trust`, review `git tpl diff`, and run `git tpl merge`.
The parent pins each child to a reviewed full commit in its `template.toml`; the corresponding template CI checks out
those same commits, and `.config/git.tpl.toml` records the revisions used by each provider rendering.
Update the child template and its tests first, advance both pins together with the parent composition test, then have
provider authors review `git tpl diff --json` before `git tpl merge --json` to adopt the new scaffold.
Changing a template does not change an existing plugin until its owner explicitly updates and merges its rendered ref.

The checked-in `plugins/example` project is a packaging-tooling fixture run by Core CI;
standalone providers use the workflows produced by the pinned parent template rather than copied example workflows.
A standalone provider publishes its own versioned artifact;
its reviewed catalogue entry lists the artifact URL and digest explicitly.
A direct GitHub repository URL is for a standalone repository whose releases publish exactly one plugin ID;
Core's releases are not provider releases; discover providers through `plugins.json`.
GitHub's release-asset SHA-256 protects a direct download's integrity; it is not an ed25519 publisher signature.
With `plugins.trust = "required"`, use a signed catalogue entry instead of an unsigned direct release.
The signature attests the digest using a configured trusted public key; neither a matching digest nor inclusion in the
curated catalogue independently proves that the publisher is trusted.
Standalone provider release workflows validate the manifest, locked source dependencies, conformance and exact artifact
before publishing a versioned tag; standalone tags must point to reviewed commits on `main`.
Moving the same project into a standalone repository keeps `plugin.id` and module IDs stable;
update `plugin.repository` and the discovery catalogue through review before releasing from the new repository.

## Standalone development and compatibility

A provider repository has an immutable `plugin.toml` descriptor, independent typed source/integration implementations and
fixtures, and its own CI and release workflows. The release `manifest.toml` exists inside the archive only;
older providers may still retain a local manifest seed until they migrate.
The parent manifest's `memcastle` semver range must match the source and integration manifests' Core requirements;
source `contract` must also match its module manifest's versioned WIT contract.
Module IDs stay stable across repository moves; module versions and the parent release version are separate.
Keep the lower bound at the oldest Core actually supported and narrow the upper bound when a breaking Core contract
requires it. Review contract and WIT changes against both generated modules before advertising a new range.

For a generated checkout, build modules using its `ci.yaml` steps and run `memcastle plugin package` from its root;
commit source `Cargo.lock` files, review the archive digest, and leave `plugin.toml` unchanged.
The `memcastle plugin inspect --json`, `memcastle plugin package`, and
`memcastle plugin test dist/ID-VERSION.tar.gz` authoring commands are provider-independent.
`plugin package` generates the complete manifest **only inside the archive** and writes an external archive checksum
from built modules, refusing unsafe or unlisted files. Both inspect and package default to `./plugin.toml`.
An exact `ID/vVERSION` tag yields that version; untagged Git revisions use an identifiable development version,
and `--version` overrides Git when packaging without a repository;
it does not build source components or agent bundles. `plugin test` uses a disposable palace and does not register
agent integrations; provider-specific agent and adoption tests remain in the standalone repository.
Provider-owned JavaScript adoption and agent-installation checks remain separate; generic `plugin test` checks source
activation without changing the user's palace or agent configuration.
The shared plugin validation workflow always tests source conformance and archive installation/activation against the
tested Core `main` prerelease.
Callers may additionally name plugin-capable release trees such as `0.5` (the latest published patch), or an exact
stable version; each gets the same required integration checks.
Stable Core 0.4.1 does not support plugin installation, so it cannot be added to this matrix.
Core CI builds and tests the `plugins/example` tooling fixture on each pull request and main push.
For cross-repository development, put a Core binary from its worktree on PATH and run the same smoke check;
standalone provider CI and its release workflow build neither unrelated providers nor Core from source.

### Shared CI and development Core

Every successful Core CI run on `main` that passes the basic, slow, WebAssembly and other required Core jobs
publishes a Linux amd64 development binary to the fixed
[`development` prerelease](https://github.com/memcastle/memcastle/releases/tag/development).
The release contains exactly `memcastle_linux-amd64` and `SHA256SUMS`, whose sole entry covers that binary.
The release body contains `Core main SHA: <40-character SHA>`, and the binary asset's label records the same SHA.
The publisher updates the body only after both assets are uploaded, then removes all older assets.
If CI or a build fails, the previous release stays available; a failed upload or an in-progress replacement
may briefly prevent downloads until a complete pair with the advertised SHA is restored.
A superseded commit is not published when the publisher starts.
The `development` tag is a fixed discovery address, **not** a pointer to the newest source commit:
read the release body.
Stable tags, versioning, Homebrew and AUR publication remain independent of this prerelease.
Core's publishing job uses the existing GitHub App credentials in its `release` environment;
binary build jobs need only `contents: read`.

Standalone plugins own their build, packaging, versioning and release workflow.
The generated `ci.yaml` builds each source/integration and calls `plugin package`, then uploads the archive and
source components before Core's shared validation workflow.
The shared workflow downloads that artifact; it never builds or packages a plugin.
It reconstructs each source's `dist/source.wasm` fixture from the uploaded root component only after packaging,
whether the plugin uses `sources/<id>` or the legacy `modules/<id>` layout.
Caller jobs grant `contents: read` for validation and `contents: write` only where the provider publishes its release.
For example, after a provider's `package` job uploads one archive and its source components:

```yaml
permissions:
  contents: read
jobs:
  validate:
    needs: package
    strategy:
      matrix:
        core: [development]
    uses: memcastle/memcastle/.github/workflows/plugin-validation.yaml@main
    with:
      artifact-name: plugin-package
      core-version: ${{ matrix.core }}
      extra-smoke-command: node scripts/smoke_adoption.mjs
```

Callers supply one required `core-version` per matrix leg, including `development` or a published stable release tree.
The shared workflow validates exactly that one version; the caller owns its compatibility matrix.
Add `0.5` only after a plugin-capable Core release exists; there is no conditional integration-test skip.
The required `artifact-name` identifies the already-built plugin; `archive-path` remains an optional legacy input.
Without it Core demands exactly one archive below `dist/` and reports that path to the caller;
include `sources/*/source.wasm` (or legacy `modules/*/source.wasm`) so `source test --no-build` exercises those bytes.
For a release dispatched on `main`, pass `plugin-ref` with the reviewed tag so the fixture checkout matches the
exact tagged archive; normal PR/main validation defaults to the caller's commit.
The `smoke-command`, `extra-smoke-command` and `fixture-bin` inputs allow additional provider-owned checks and fixtures;
the generic `plugin test` always runs on the supplied archive before them.
The provider's release workflow decides when to validate, requires its own reviewed version tag,
and publishes the archive only after the shared validation succeeds.
The root `memcastle/memcastle@main` action places the verified binary on PATH and returns `binary`, `core_sha` and
`resolved_version`. `development` is checked against the release body, asset label and fixed-name checksum;
`0.5`/`1` choose the latest published patch in that minor/major tree before checking its exact stable release checksum.
An exact stable tag is downloaded directly; a missing release or asset fails at download time.
Each compatibility matrix leg reports its exact Core revision in the job summary.
To add another provider, supply its own package/check and archive install scripts,
select compatible stable tags explicitly, and keep provider-specific fixture setup or legacy migration smokes
in the caller's inputs.
