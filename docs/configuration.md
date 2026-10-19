# Configuration

MemCastle runs with no configuration at all.
Everything below is for changing where it keeps things, or how it behaves.

## Where files live

MemCastle follows the **Unix XDG Base Directory convention** on Linux and macOS alike.
macOS does **not** use `~/Library/Application Support`: both platforms use the same paths,
so a dotfiles repository or a shell profile written once works on both.

| What | Default | Relocated by |
|---|---|---|
| Config file | `~/.config/memcastle/config.toml` | `XDG_CONFIG_HOME`, or `--config` / `MEMCASTLE_CONFIG` |
| Palace (persistent data) | `~/.local/share/memcastle/default` | `XDG_DATA_HOME`, or `--palace` / `MEMCASTLE_PALACE_PATH` / `palace.path` |
| Daemon registry (runtime state) | `~/.local/state/memcastle/run/` | `XDG_STATE_HOME` |
| OAuth credential files (only when no keyring is used) | `~/.local/share/memcastle/credentials/` | `XDG_DATA_HOME`, or `credentials.dir` |
| Package assets (not user data) | `<prefix>/share/memcastle`, or none | `--assets-dir` / `MEMCASTLE_ASSETS_DIR` / `assets.dir` |

The last row is not a place you keep anything.
It is where an OS package installs read-only files, the bundled sources, the agent integrations and the skills,
and it is never under the XDG directories.
See [Runtime assets](#runtime-assets).

The XDG variables are honoured as the specification describes:
an unset, empty or relative value is ignored, and the default under your home directory is used instead.
For example, with `XDG_DATA_HOME=/srv/data` the palace is `/srv/data/memcastle/default`.

A **project** has its own, separate file, `.config/memcastle.toml` in the project's directory.
It is not this configuration: it holds no daemon setting, and it says which wing and room a project's memory belongs to
(see [Project configuration](project-config.md)).

The config file is optional.
If the default file does not exist, built-in defaults are used.
A file named explicitly with `--config` must exist, because a typo there would otherwise be silently ignored.

### Palace and data path selection

A palace is one directory.
With the embedded store, the SurrealDB files live in its `db/` subdirectory.
The path is chosen by, highest precedence first: `--palace`, `MEMCASTLE_PALACE_PATH`, `palace.path` in the config file,
then `$XDG_DATA_HOME/memcastle/default`.
It must be an absolute path; a relative one is rejected at startup, because a daemon and a client started
from different directories would disagree about which palace they mean.

Every command resolves the palace the same way, so `serve`, `status`, `migrate`, `daemon stop` and the rest agree on it.
A client finds a running daemon through the registry file, which is keyed by the palace path.
`daemon start` and `daemon restart` pass the palace they resolved on to the new daemon.

The registry is runtime metadata, not palace data, so it lives in the state directory rather than beside the palace.
Backing up or copying a palace never carries a stale daemon record with it.
Each palace has its own file, `$XDG_STATE_HOME/memcastle/run/<palace-hash>/daemon.json`,
where `<palace-hash>` is derived from the palace's canonical path.
If no home directory can be determined, the registry falls back to `memcastle/run` under the system temporary directory.
[Storage and data](storage.md#the-registry-file) describes the file.

## Precedence

From lowest to highest, a later layer overrides an earlier one:

1. Built-in defaults.
2. The config file.
3. `MEMCASTLE_*` environment variables.
4. Command-line flags.

A malformed environment variable is an error naming the variable; it is never silently ignored.
The resolved configuration is validated once all layers are applied.
Unknown keys in the TOML file are currently ignored by normal configuration loading.
Run `memcastle doctor` to see warnings for unknown keys, including misspelled nested settings.

Logging has one more input.
Its precedence, highest first, is `MEMCASTLE_LOG`, `RUST_LOG`, the `-v` / `-vv` flags, then `logging.level`.
`MEMCASTLE_LOG`, `RUST_LOG` and `logging.level` accept a level (`error`, `warn`, `info`, `debug`, `trace`)
or a filter directive such as `memcastle=debug,warn`.

## Config file

TOML.
Every key is optional, and a file may set only some of them.

```toml
[palace]
path = "/home/alice/.local/share/memcastle/default"

# Global fallback for all evidence, including sources not yet installed.
[preferences]
default = "normal"              # "low", "normal", or "high"
[preferences.sources.pi]
level = "high"
[[preferences.sources.pi.criteria]]
path = "working_directory"      # exact JSON metadata key, or dot-separated nested keys
equals = "/home/alice/work"     # string, number or boolean; exact match
level = "high"

# Overrides for the palace selected above; unspecified settings inherit.
[palace.preferences.sources.opencode]
level = "high"

[server]
bind = "127.0.0.1"
port = 8420

[logging]
level = "info"
# "text" for a terminal, "json" (one object per line) for journald and log shippers.
format = "text"

# Optional bearer-token authentication. Prefer MEMCASTLE_AUTH_TOKEN to `token` here,
# so the secret stays out of the file.
[auth]
enabled = false

# Defaults for `memcastle db start`. Nothing here opens the endpoint; see "The database admin endpoint".
[db]
bind = "127.0.0.1"
port = 8000
allow_remote = false
allowed_origins = []

# The listener that `webhook` triggers are delivered to; see "Triggers". Off by default: nothing listens until it is
# enabled here *and* a webhook trigger is enabled.
[webhook]
enable = false
bind = "127.0.0.1"
port = 8787
allow_remote = false
max_body_bytes = 1048576
max_concurrent = 16

# Where embeddings come from. Without a provider search stays lexical; see "Embeddings".
[embeddings]
provider = "none"          # "none", "command" or "http"
# command = ["/usr/local/bin/embed"]
# url = "http://localhost:11434/v1"
# model = "nomic-embed-text"
timeout_secs = 30
batch_size = 16

# Where entities and relationships are extracted from mined text. Off by default; see "Extraction".
[extraction]
provider = "none"          # "none", "heuristic", "command" or "http"
# command = ["/usr/local/bin/extract"]
# url = "http://localhost:11434/v1"
# model = "llama3.1"
timeout_secs = 120
batch_size = 8
max_entities = 32          # kept from one drawer
max_relations = 64
min_confidence = 0.3       # relationships below this are dropped

# How eagerly duplicate memories and entity spellings are recognised; see "Deduplication".
[dedup]
enabled = true             # false stores every drawer and links nothing
near_threshold = 0.9       # similarity, 0.5 to 1, at which a drawer is linked as a likely copy
entity_fuzzy = true        # a unique one-character typo converges on the entity it resembles

# How mining cuts and bounds what it reads; see "Mining sources".
[mining]
chunk_chars = 6000          # characters per drawer; a longer document becomes several drawers
max_file_bytes = 2097152    # directory mining skips files larger than this
max_documents = 0           # 0 means unlimited; a positive value caps one job, whose next run resumes the cursor
# Installed WebAssembly sources (see "Writing a mining source"): where they live, and what each call may use.
# sources_dir = "/home/alice/.local/share/memcastle/sources"
source_memory_mib = 256     # the most memory one call into an installed source may use
source_timeout_secs = 60    # the longest one call may run; a source's own limits can only lower these
# Where `memcastle source search|install <name>|update` look: an https URL, a file:// URL or an absolute path to a
# memcastle-index.json (or a directory holding one). The official registry by default; a list here replaces it, and
# `[]` means none. The default is https://memcastle.github.io/registry.json.
# It is read only when you run one of those commands.
# registries = ["https://example.org/memcastle/index.json"]
# The GitHub API a registry entry that names a repository is resolved through (see "Publishing and installing sources").
# github_api_url = "https://api.github.com"
# "optional" installs an unsigned package and refuses a bad signature from a key you trust; "required" installs only
# what a key in `trusted_keys` signed. Keys are the public keys `memcastle source keygen` prints.
trust = "optional"
# trusted_keys = ["Zm9vYmFy..."]

[plugins]
# Plugin catalogues are independent of the legacy source registry. They are read on explicit search requests only.
# registries = ["https://memcastle.github.io/plugins.json"]
# trust = "optional"        # set "required" to accept only releases signed by a trusted key
# trusted_keys = []         # ed25519 public keys (same format as mining.trusted_keys)
# Immutable plugin versions live here by default: $XDG_DATA_HOME/memcastle/plugins.
# dir = "/home/alice/.local/share/memcastle/plugins"

# Where the tokens of a source signed in with OAuth are kept; see "Credentials".
[credentials]
backend = "auto"           # "auto" (the platform keyring, else a file), "keyring" or "file"
# dir = "/home/alice/.local/share/memcastle/credentials"   # for the file store

# What the daemon mines, by name; see "Miners". Written by `memcastle miner set`, or by hand.
[[miners]]
name = "docs"
source = "directory"
locator = "/home/alice/src/docs"

# Only to serve assets from somewhere other than the installed or embedded ones: a package unpacked elsewhere, or a
# checkout of the repository (which holds sources/, integrations/, skills/ and web/dist).
[assets]
dir = "/home/alice/src/memcastle"

# The web UI, served under /ui. Off by default; see "Web UI".
[web]
enable = false

[jobs]
max_concurrency = 4
background_concurrency = 2 # mine/embed/extract jobs; leaves a slot for other jobs
drain_timeout_secs = 10
lease_ttl_secs = 30

# Embedded SurrealKV under palace.path (the default), or a remote SurrealDB.
[store]
mode = "embedded"
# When an embedded palace forces its writes to disk: "every" (each commit, the default),
# "never" (leave it to the operating system) or an interval over 100ms such as "500ms", "5s" or "1m".
sync = "every"
```

`jobs.background_concurrency` limits mine, embed and extract jobs independently of `jobs.max_concurrency`.
When the total limit is greater than one, the effective background limit is capped at one less than the total;
with a total limit of one, one background job may run.
Lower the background limit if API reads slow down during large mines or embedding sweeps;
raise it only when the datastore and CPU have spare capacity.

### Source preferences

`[preferences]` supplies the global fallback; each `[preferences.sources.<connector>]` optionally sets a level and
`[[preferences.sources.<connector>.criteria]]` can select a level for a matching source-document metadata value.
Connector names are mining adapter names (`pi`, `opencode`, `directory`, or an installed source), not broad drawer kinds.
There is no shared criterion vocabulary: use JSON keys actually exposed by that connector, including nested paths
separated by dots; keys must consist of letters, digits, underscores or hyphens.
Rules compare JSON strings, numbers and booleans exactly, without case folding or coercion.
The longest matching path wins; equal-depth matches use their first declaration.
An unknown connector or a drawer without a mining origin uses the default;
an absent key or unmatched value uses its connector level, or the default when no level was set.
An unknown connector *in configuration* is allowed so a source can be configured before it is installed.

The global configuration can also contain `[palace.preferences]` with the same keys and nested connector tables.
Optionally put a preferences-only file at `<palace.path>/preferences.toml`:

```toml
[preferences]
default = "normal"
[preferences.sources.pi]
level = "normal"  # explicitly replaces a global `high`
criteria = []     # explicitly clears inherited connector criteria
```

Precedence is built-in `normal`, global `[preferences]`, global `[palace.preferences]`, then the palace-local file.
Only supplied fields replace earlier fields; a connector level does not erase inherited criteria.
The selected `palace.path` (including environment and `--palace` overrides) determines which local file is read.
This file is distinct from a project's `.config/memcastle.toml`.
Changes take effect on daemon restart; previous source-document metadata is retained on the originating drawer.

Levels shape ranks within relevant candidates and the read-side hint on unresolved conflicting facts.
They do not exclude a source, guarantee correctness, raise an extractor's confidence, or reopen superseded memory.
Explicit corrections and validity still win; relevance, original confidence and freshness remain decisive.
Search-hit `signals.preference` reports the effective level, connector and matched criterion for inspection.
See [Source preferences](adr/048-source-preferences.md) for the ranking and inheritance decision.

A remote store also needs a URL and credentials:

```toml
[store]
mode = "remote"
url = "ws://localhost:8000"
namespace = "memcastle"
database = "main"
username = "root"
password = "..."
```

Remote store settings are file-only; there is no environment variable for them.
Keep secrets out of version control: put this file outside any repository, and restrict it with `chmod 600`.

## Environment variables

| Setting (TOML key) | Environment variable | Default |
|---|---|---|
| `palace.path` | `MEMCASTLE_PALACE_PATH` | `~/.local/share/memcastle/default` |
| `server.bind` (an IP address) | `MEMCASTLE_BIND` | `127.0.0.1` |
| `server.port` (0 to 65535) | `MEMCASTLE_PORT` | `8420` |
| `assets.dir` (an absolute path) | `MEMCASTLE_ASSETS_DIR` | none: installed, then embedded assets |
| `web.enable` (`true` or `false`) | `MEMCASTLE_WEB_ENABLE` | `false` |
| `logging.level` | `MEMCASTLE_LOG` | `info` |
| `logging.format` | `MEMCASTLE_LOG_FORMAT` | `text` (`text` or `json`) |
| `jobs.max_concurrency` (at least 1) | `MEMCASTLE_JOBS_MAX_CONCURRENCY` | `4` |
| `jobs.background_concurrency` (at least 1) | `MEMCASTLE_JOBS_BACKGROUND_CONCURRENCY` | `2` |
| `jobs.drain_timeout_secs` (1 to 86400) | `MEMCASTLE_JOBS_DRAIN_TIMEOUT_SECS` | `10` |
| `jobs.lease_ttl_secs` (3 to 86400) | `MEMCASTLE_JOBS_LEASE_TTL_SECS` | `30` |
| `auth.enabled` (`true` or `false`) | `MEMCASTLE_AUTH_ENABLED` | `false` |
| `auth.token` (at least 16 characters) | `MEMCASTLE_AUTH_TOKEN` | none |
| `db.bind` (an IP address) | `MEMCASTLE_DB_BIND` | `127.0.0.1` |
| `db.port` (0 to 65535) | `MEMCASTLE_DB_PORT` | `8000` |
| `db.allow_remote` (`true` or `false`) | `MEMCASTLE_DB_ALLOW_REMOTE` | `false` |
| `db.allowed_origins` (a list) | `MEMCASTLE_DB_ALLOWED_ORIGINS` (comma-separated) | none |
| `webhook.enable` (`true` or `false`) | `MEMCASTLE_WEBHOOK_ENABLE` | `false` |
| `webhook.bind` (an IP address) | `MEMCASTLE_WEBHOOK_BIND` | `127.0.0.1` |
| `webhook.port` (0 to 65535) | `MEMCASTLE_WEBHOOK_PORT` | `8787` |
| `webhook.allow_remote` (`true` or `false`) | `MEMCASTLE_WEBHOOK_ALLOW_REMOTE` | `false` |
| `embeddings.provider` (`none`, `command`, `http`, `openai` or `external`) | `MEMCASTLE_EMBEDDINGS_PROVIDER` | `none` |
| `embeddings.url` (an `http://` or `https://` URL) | `MEMCASTLE_EMBEDDINGS_URL` | none |
| `embeddings.model` | `MEMCASTLE_EMBEDDINGS_MODEL` | none |
| `embeddings.api_key` | `MEMCASTLE_EMBEDDINGS_API_KEY` | none |
| `embeddings.timeout_secs` (1 to 3600) | `MEMCASTLE_EMBEDDINGS_TIMEOUT_SECS` | `30` |
| `embeddings.batch_size` (1 to 1024) | none | `16` |
| `embeddings.command` (a list) | none | none |
| `embeddings.dimensions` (positive integer, legacy single-provider form) | none | `768` |
| `embeddings.default` (named profile) | none | legacy `default` when a single provider is configured |
| `embeddings.providers.<name>.type` (`command`, `openai`, `http`, `plugin` or `external`) | none | none |
| `embeddings.providers.<name>.url` (`base_url` is an alias) | none | none |
| `embeddings.providers.<name>.plugin`, `.module`, `.settings`, `.credential` (plugin connections) | none | none |
| `embeddings.profiles.<name>.provider`, `.model`, `.dimensions` | none | none |
| `extraction.provider` (`none`, `heuristic`, `command` or `http`) | `MEMCASTLE_EXTRACTION_PROVIDER` | `none` |
| `extraction.url` (an `http://` or `https://` URL) | `MEMCASTLE_EXTRACTION_URL` | none |
| `extraction.model` | `MEMCASTLE_EXTRACTION_MODEL` | none |
| `extraction.api_key` | `MEMCASTLE_EXTRACTION_API_KEY` | none |
| `extraction.timeout_secs` (1 to 3600) | `MEMCASTLE_EXTRACTION_TIMEOUT_SECS` | `120` |
| `extraction.batch_size` (1 to 256) | `MEMCASTLE_EXTRACTION_BATCH_SIZE` | `8` |
| `extraction.min_confidence` (0 to 1) | `MEMCASTLE_EXTRACTION_MIN_CONFIDENCE` | `0.3` |
| `extraction.max_entities` (1 to 1000) | none | `32` |
| `extraction.max_relations` (1 to 10000) | none | `64` |
| `extraction.command` (a list) | none | none |
| `dedup.enabled` (`true` or `false`) | `MEMCASTLE_DEDUP_ENABLED` | `true` |
| `dedup.near_threshold` (0.5 to 1) | `MEMCASTLE_DEDUP_NEAR_THRESHOLD` | `0.9` |
| `dedup.entity_fuzzy` (`true` or `false`) | `MEMCASTLE_DEDUP_ENTITY_FUZZY` | `true` |
| `mining.chunk_chars` (200 to 100000) | `MEMCASTLE_MINING_CHUNK_CHARS` | `6000` |
| `mining.max_file_bytes` (1 to 67108864) | `MEMCASTLE_MINING_MAX_FILE_BYTES` | `2097152` |
| `mining.max_documents` (0 or 1 to 1000000) | `MEMCASTLE_MINING_MAX_DOCUMENTS` | `0` (unlimited) |
| `mining.sources_dir` (an absolute path) | `MEMCASTLE_MINING_SOURCES_DIR` | `$XDG_DATA_HOME/memcastle/sources` |
| `mining.source_memory_mib` (16 to 4096) | `MEMCASTLE_MINING_SOURCE_MEMORY_MIB` | `256` |
| `mining.source_timeout_secs` (1 to 3600) | `MEMCASTLE_MINING_SOURCE_TIMEOUT_SECS` | `60` |
| `mining.registries` (a list of URLs or absolute paths) | `MEMCASTLE_MINING_REGISTRIES` (comma-separated; empty for none) | the [official registry](publishing-sources.md#the-official-registry) |
| `mining.github_api_url` (an `https://` URL) | `MEMCASTLE_MINING_GITHUB_API_URL` | `https://api.github.com` |
| none (never in the file) | `GH_TOKEN`, else `GITHUB_TOKEN` | no token: GitHub's unauthenticated rate limit applies |
| `mining.trust` (`optional` or `required`) | `MEMCASTLE_MINING_TRUST` | `optional` |
| `mining.trusted_keys` (a list of base64 public keys) | `MEMCASTLE_MINING_TRUSTED_KEYS` (comma-separated) | none |
| `mining.bundled_dir` (an absolute path) | `MEMCASTLE_MINING_BUNDLED_DIR` | `share/memcastle/sources` of the installation |
| `plugins.dir` (an absolute path) | none | `$XDG_DATA_HOME/memcastle/plugins` |
| `plugins.registries` (a list of URLs or absolute paths) | none | `https://memcastle.github.io/plugins.json` |
| `plugins.trust` (`optional` or `required`) | none | `optional` |
| `plugins.trusted_keys` (a list of base64 public keys) | none | none |
| `credentials.backend` (`auto`, `keyring` or `file`) | `MEMCASTLE_CREDENTIALS_BACKEND` | `auto` |
| `credentials.dir` (an absolute path) | `MEMCASTLE_CREDENTIALS_DIR` | `$XDG_DATA_HOME/memcastle/credentials` |
| `store.sync` (`every`, `never` or an interval over 100ms) | `MEMCASTLE_STORE_SYNC` | `every` |
| `store.mode` and remote settings | none | `embedded` |

`auth.token` is a secret, and is handled as one: MemCastle never logs it, prints it, serialises it, or writes it anywhere,
and the client commands read it from the environment or the config file, never from a flag.
Whitespace around `MEMCASTLE_AUTH_TOKEN` is ignored,
but a token in the config file with leading or trailing whitespace is refused at start,
because a client's token is trimmed too and the two could never match.
See [Authentication](authentication.md).

`embeddings.api_key` and `extraction.api_key` are secrets too: they are never logged, printed or serialised,
and an invalid value is never echoed.

Some variables are read by the command line rather than the config file:

| Variable | Equivalent flag |
|---|---|
| `MEMCASTLE_CONFIG` | `--config` |
| `MEMCASTLE_MODE` | `--mode` |
| `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` | none |
| `RUST_LOG` | none |

## Command-line flags

| Flag | Applies to | Overrides |
|---|---|---|
| `--config <FILE>` | every command | the default config file location |
| `--palace <PATH>` | every command | `palace.path`, `MEMCASTLE_PALACE_PATH` |
| `--bind <IP>` | `serve`, `daemon start`, `daemon restart` | `server.bind`, `MEMCASTLE_BIND` |
| `--port <PORT>` | `serve`, `daemon start`, `daemon restart` | `server.port`, `MEMCASTLE_PORT` |
| `--assets-dir <DIR>` | `serve`, `daemon start`, `daemon restart`, `integration list`, `install` and `update` | `assets.dir`, `MEMCASTLE_ASSETS_DIR` |
| `--mode <MODE>` | every command (acted on by client commands) | the memory mode of the session |
| `-v`, `-vv` | every command | the log level of memcastle itself |

## The listener: address and port

The daemon serves the REST API and MCP on one TCP listener, `127.0.0.1` port `8420` unless configured otherwise:

```sh
memcastle serve --bind 127.0.0.1 --port 8787
```

The address and the port are separate settings, so either can be changed alone.
Each is chosen by, highest precedence first: the flag, the environment variable, the config file, the default.
`serve`, `daemon start`, `daemon restart` and a supervisor such as systemd all start the daemon through the same path,
so the same three sources work everywhere.
Client commands (`status`, `search`, `job` and the rest) read the config file and the environment,
but not the flags, so a daemon started on a non-default port with `--port` is found through its registry file.
`daemon start` and `daemon restart` pass their `--bind`, `--port` and `--assets-dir` on to the new daemon.

`memcastle status` shows which of the two it used (`endpoint_source`: `registry` or `config`).
It exits 0 for a healthy daemon, 1 for a degraded one and 3 when none is running,
and prints JSON when piped (or with `--json`) for scripts:

```sh
memcastle status                      # human-readable report in a terminal, JSON in a pipe
memcastle status --json | jq .daemon.datastore
memcastle status || echo "exit $?"    # 3 means not running
```

`server.bind` is an IP address, IPv4 or IPv6, and not a host name.
A `host:port` value, as `bind` accepted before the port became its own setting, is refused with a message naming the port
setting to use instead.
Port `0` asks the OS for a free port, which is useful for tests and scripts;
the port actually chosen is in the daemon's registry file and its log.

The default never listens on all network interfaces.
Authentication is off unless you enable it, so use a non-loopback address such as `0.0.0.0` only on a network you trust,
or with [authentication](authentication.md) enabled.
The daemon logs a warning when it listens beyond loopback without it.
The MCP endpoint additionally refuses a non-loopback `Host` header, see [Authentication](authentication.md#exposing-the-daemon).

The listener is bound before anything that changes state: the runtime assets are only located, not yet used.
If the address is taken, needs privileges, or does not exist on this machine,
the start fails with `memcastle::server::bind_failed`, naming the address and what to change,
and has not created the palace, migrated it or touched its jobs.

## Embeddings

Semantic and hybrid search rank by vectors, and a vector has to come from a model.
MemCastle links no model; `[embeddings]` selects how to reach one and keeps credentials out of profile settings.
Choose a dedicated embedding model that answers an embeddings API request with a numeric vector.
A chat/completion model or an authenticated chat CLI session does not necessarily provide embeddings.
Without it the palace works exactly as before and `search` is lexical.
A vector is *derived* data: it is stored beside the drawer, never replaces its content, and can be recomputed at any time.

The legacy single-provider configuration defaults to 768 dimensions; set `dimensions` for another model.
The daemon requests each profile's configured dimensions and rejects a different-sized response with
`memcastle::embed::dimension_mismatch`.
Vectors for different profiles have separate records keyed by a generation-pipeline fingerprint, including the
connection identity, model, dimensions and truncation version.
Changing any of those settings creates a new space, rather than silently reusing incompatible vectors.
The first space to claim a schema-declared 768-, 1536- or 3072-dimensional HNSW tier owns that tier permanently;
other spaces, including models with the same dimensions, use slower exact profile-filtered ranking.
Other positive dimensions also use exact ranking, so a model change never needs runtime schema DDL.
The indexed tier cannot be reassigned by disabling a profile; choose the profile to index before the first embedding
at that dimension (usually by setting it as the default), or plan for exact searches that grow with its corpus size.
Use a versioned, immutable model identifier when the provider offers one.
If an endpoint silently changes the model behind the *same* alias, MemCastle cannot detect that from configuration:
pin the revision at the provider, or change the configured model/profile deliberately and re-embed.

### Named connections and profiles

Provider accounts share a credential across compatible connections, independently of their endpoints and models:

```toml
[provider_accounts.openai-main]
provider_id = "openai"
auth = { type = "bearer", credential = { type = "env", name = "OPENAI_API_KEY" } }

[provider_accounts.openrouter-main]
provider_id = "openrouter"
auth = { type = "bearer", credential = { type = "file", path = "/run/secrets/openrouter-key" } }

[provider_accounts.openai-budget]
provider_id = "openai"
auth = { type = "api_key", header = "x-api-key", credential = { type = "env", name = "OPENAI_OTHER_KEY" } }

[embeddings.providers.openai-direct]
type = "openai"
provider_id = "openai"
account = "openai-main"
base_url = "https://api.openai.com/v1"

[embeddings.providers.openai-second-endpoint]
type = "openai"
provider_id = "openai"
account = "openai-main"
base_url = "https://api.openai.com/v1"

[embeddings.providers.router]
type = "openai"
provider_id = "openrouter"
account = "openrouter-main"
base_url = "https://openrouter.ai/api/v1"

[embeddings.profiles.openai]
provider = "openai-direct"
model = "text-embedding-3-small"
dimensions = 1536
```

The provider ID on a connection must match the selected account; it identifies the service, while `type` identifies
the embedding protocol (OpenRouter speaks the OpenAI-compatible protocol).
More than one account per provider can separate projects or budgets; a future LLM connection can select the same
account without duplicating its credential, once an LLM provider runtime exists.
`bearer` uses `Authorization: Bearer`; `api_key` uses `x-api-key` unless a different safe `header` is configured.
An account's `credential` accepts only an environment variable in the daemon's environment or an absolute file path;
source OAuth and interactive sessions are not interchangeable provider credentials.
The daemon reads the token when the selected provider requests it, so changing the referenced token needs no restart;
changing the account definition or connection selection does.
`GET /api/config` and the dashboard show account availability, provider and auth kind without showing the reference location
or token. A missing credential fails the selected embedding request and tells you which account to check.
To migrate an existing plugin connection, move its `credential` reference into the account's `auth.credential`, remove
the inline `credential`, and set matching `provider_id` and `account` on the connection.
Existing inline plugin references, named HTTP `api_key` values and legacy `[embeddings]` settings still work.
Credential references and account IDs do not form part of a vector-space fingerprint; changing the endpoint, provider,
non-secret plugin settings or model still does. Older plugin fingerprints included a credential-reference hash, so their
vectors need an explicit re-embedding job after upgrading to this account boundary.
See [ADR-052](adr/052-reusable-provider-accounts.md) for the interactive-authentication extension boundary.

```toml
[embeddings]
default = "local"

[embeddings.providers.ollama]
type = "openai"
base_url = "http://localhost:11434/v1"

[embeddings.providers.router]
type = "openai"
provider_id = "openrouter"
account = "openrouter-main"
base_url = "https://openrouter.ai/api/v1"

[embeddings.profiles.local]
provider = "ollama"
model = "qwen3-embedding"
dimensions = 768

[embeddings.profiles.bulk]
provider = "router"
model = "openai/text-embedding-3-small"
dimensions = 1536
```

The `openai` type implements the OpenAI embeddings protocol, including compatible endpoints.
`http` remains available for the legacy single-provider form.
No OpenAI product plugin is needed to call OpenAI or an OpenAI-compatible service such as OpenRouter.
Use the provider's documented embedding-capable model ID; a model listed for chat alone may reject `/embeddings`.
For hosted connections, set the account credential in the *daemon's* environment or the referenced file, not in a
private CLI credential store.
With the daemon running, validate each configured profile independently before submitting a sweep:

```sh
memcastle doctor
memcastle embed test --profile local --text 'A sample sentence to embed.'
memcastle embed test --profile bulk --text 'A sample sentence to embed.'
memcastle embed --profile local
```

The test is an explicit provider call: it transmits only the supplied sample to the selected endpoint, validates one
finite vector of the profile's configured dimensions, and creates neither a job nor a stored embedding.
Omit `--profile` to test the global default; `doctor` inspects configuration and local prerequisites but never sends text
or confirms remote authentication, model support or endpoint availability.
If the server rejects credentials, check the selected account; if it returns 404, check the base URL and `/embeddings`
path before assuming the model is unavailable.
A dimension mismatch means the provider's returned vector cannot enter the selected space: correct the profile dimensions
or use a model that returns the configured length, then deliberately re-embed into the new space.
An explicit `memcastle embed --profile NAME` selects that profile for its job; after a miner run its configured
`embedding_profile` determines the source-scoped automatic sweep; otherwise the global default is used.
An installed plugin with an explicit `embedding_provider` module can instead be selected through the same named
connection/profile model:

```toml
[embeddings]
default = "vendor"

[embeddings.providers.vendor-api]
type = "plugin"
plugin = "vendor-plugin"
module = "vendor-embeddings"
timeout_secs = 30
batch_size = 16
credential = { type = "env", name = "VENDOR_EMBED_TOKEN" }
settings = { region = "eu" }

[embeddings.profiles.vendor]
provider = "vendor-api"
model = "vendor-embedding-v2"
dimensions = 1536
```

The plugin's module manifest must advertise the exact model and dimensions and declare credential permission;
its configuration schema validates `settings` once installed.
Only non-secret settings go into `settings`: the host resolves an `env` or `file` credential reference when a selected
component requests it, never from an unrelated source, integration or profile.
The component runs with bounded memory and time and receives network access only after explicit plugin-install consent.
An uninstalled module leaves its profile unavailable but does not prevent the daemon or other profiles from working.
Install it through `memcastle plugin install`, then select it with `memcastle embed --profile vendor`, a miner's
`embedding_profile = "vendor"`, or a profile-aware search; the global default is used when no override was given.
Changing a module's component or non-secret settings creates a new space: run a new embedding job to populate it.
For caller-computed vectors without a daemon provider, use `type = "external"` for a named connection and select its
profile explicitly on upload and query; the declared model and dimensions identify the space.
Do not use one external profile for vectors from different models.
The existing single-provider keys under `[embeddings]` map to a profile named `default` when that provider is enabled.
If only named profiles are configured, set `embeddings.default` to enable automatic sweeps and unqualified semantic
queries; otherwise select a profile explicitly for each job or search.
Old vectors in `drawer.embedding` have no verifiable model identity: they are retained but excluded from profile search.
Run `memcastle embed --profile NAME` to populate a new space.

### The `command` provider

```toml
[embeddings]
provider = "command"
command = ["/usr/local/bin/embed", "--quiet"]
model = "my-model"      # passed to the program, optional
```

MemCastle runs the program, without a shell, once per batch, writes one JSON object to its standard input
and reads one from its standard output:

```json
{"model": "my-model", "dimension": 768, "texts": ["first text", "second text"]}
{"embeddings": [[0.01, 0.02, "… 768 numbers"], [0.03, 0.04, "…"]]}
```

The program returns one vector per text, in order, and exits `0`.
A non-zero exit, output that is not that JSON, or no answer within `timeout_secs` fails the call, and the program is killed.
This is a lightweight generic provider for a local program with its own client library or credential handling:
the program owns those credentials, MemCastle never sees them, and the daemon's own `MEMCASTLE_*` variables
(its auth token included) are removed from the program's environment.
Everything else is inherited, so `PATH` and the program's own variables work.
For example, a local adapter can read the JSON `texts` from standard input, invoke a documented embedding-capable CLI
or SDK for each text, and print `{"embeddings":[...]}` to standard output; configure it as a named `command` connection
and pin the CLI's actual embedding model and dimensions in a named profile.
Do not pass ordinary completion output off as embedding vectors or read another CLI's private authentication files.

### The `http` provider

```toml
[embeddings]
provider = "http"
url = "http://localhost:11434/v1"
model = "nomic-embed-text"
```

MemCastle posts to `{url}/embeddings` in the OpenAI format, so OpenAI, Ollama, llama.cpp's server and vLLM all work.
A local server needs no key.
A hosted one takes `MEMCASTLE_EMBEDDINGS_API_KEY`, sent as a bearer token; prefer that to `api_key` in the file.

### How embedding happens

- The daemon embeds in the background.
  After anything that writes drawers (mining, a checkpoint, a diary entry, a created drawer) and once at startup,
  it queues a low-priority `embed` job that embeds every drawer missing a vector in the default profile.
  Queued sweeps are coalesced, so a burst of writes leaves one waiting job.
  `memcastle embed` queues one by hand, for example after you first configure a provider.
  `memcastle embed --profile bulk --refresh stale --wing project` selects a profile and refreshes missing or stale inputs;
  `--refresh all` re-embeds every drawer in scope, while the default is `missing`.
  Jobs persist their selected model and generation settings and fail with an actionable error if a restarted daemon has
  changed them; submit a new job after changing a profile.
- A drawer is embedded from its first 8,000 characters, because providers cap their input.
  Its content is stored whole and verbatim whatever the provider sees.
- A query is embedded on the fly.
  If the provider fails, a default `auto` search falls back to lexical and logs a warning,
  while an explicit `semantic` or `hybrid` search fails with `memcastle::embed::failed`.
- You can skip the provider altogether and send vectors yourself, see
  [Searching](mcp-and-api.md#searching).

## Extraction

The knowledge graph links entities (people, projects, tools, ...) to each other and to the drawers that mention them,
and graph-aware search follows those links.
The `[extraction]` section says how MemCastle finds them in mined text.
Nothing is extracted by default, because two of the providers send mined text somewhere else:
choose one on purpose.
Extraction is *derived* data: it adds entities, links and relationships beside the drawers and never rewrites,
supersedes or deletes one.
See [ADR-024](adr/024-entity-extraction-as-an-enrich-job.md).

| `provider` | What reads the text | Leaves the process? |
|---|---|---|
| `none` | nothing | no |
| `heuristic` | a small built-in extractor | no |
| `command` | a program you run | whatever it does |
| `http` | an OpenAI-compatible chat endpoint | yes, to that endpoint |

### The `heuristic` extractor

```toml
[extraction]
provider = "heuristic"
```

Deterministic, with nothing to configure and no model.
It finds runs of capitalised words (`Ada Lovelace`), `@handles` and `` `code spans` ``, and relates two of them when the
words between them in one sentence are a known phrase (`works on`, `is a member of`, `depends on`, `uses`, `owns`,
`is part of`, `is based in`, ...).
It never guesses: two names that merely share a sentence are not related, and a sentence it does not recognise yields
entities and no relationship.
For more than that, use a model.

### The `command` extractor

```toml
[extraction]
provider = "command"
command = ["/usr/local/bin/extract"]
model = "my-model"      # passed to the program, optional
```

MemCastle runs the program, without a shell, once per batch, writes one JSON object to its standard input
and reads one from its standard output:

```json
{"model": "my-model",
 "vocabulary": {"kinds": ["person", "…"], "predicates": ["works_on", "…"]},
 "texts": ["first text", "second text"]}
{"extractions": [
  {"entities": [{"name": "Ada", "kind": "person"}, {"name": "MemCastle", "kind": "project"}],
   "relations": [{"subject": "Ada", "predicate": "works_on", "object": "MemCastle", "confidence": 0.9}]},
  {"entities": [], "relations": []}]}
```

One extraction per text, in order, and exit `0`.
When source preferences are configured, the request also contains `"preferences"`, an array aligned with `"texts"`.
Each entry has `level`, optional `source` and optional matched `criterion`; without preferences the original request
shape is unchanged.
A preference is a cue for evidence selection, not permission to invent a fact or raise its confidence.
A non-zero exit, output that is not that JSON, or no answer within `timeout_secs` fails the call, and the program is killed.
The program owns the model and any credential, and the daemon's own `MEMCASTLE_*` variables are removed from its environment.

### The `http` extractor

```toml
[extraction]
provider = "http"
url = "http://localhost:11434/v1"
model = "llama3.1"
```

MemCastle posts to `{url}/chat/completions` in the OpenAI format, one request per drawer, telling the model the
vocabulary and asking for JSON.
With configured source preferences, an additional system message supplies the effective level, connector and matching
criterion, and explicitly tells the model not to infer truth from that context.
A hosted endpoint takes `MEMCASTLE_EXTRACTION_API_KEY`, sent as a bearer token.
An endpoint that cannot be reached or answers with an error fails the job.
A reply that is not the JSON asked for is read as "nothing found" and logged, so one odd answer cannot block the drawers
behind it.

### What is kept

Whatever a provider says is read into a closed vocabulary, so the graph does not fill with synonyms.
Entity kinds are `person`, `organization`, `project`, `tool`, `place`, `concept` and `other`;
predicates are `works_on`, `member_of`, `depends_on`, `uses`, `owns`, `part_of`, `located_in` and `related_to`.
Anything else becomes `other` or `related_to`.
A relationship needs both ends among the entities of the same answer, and is dropped when it points at itself,
has no sensible confidence or falls below `min_confidence`.
`max_entities` and `max_relations` bound what one drawer contributes.

### How extraction happens

- The daemon extracts in the background.
  After a mining job completes, and once at startup, it queues a low-priority `extract` job,
  coalesced like the embedding sweep.
  `memcastle extract` queues one by hand.
- Only mined drawers (they carry a source origin) and [notes](cli.md#note) are read, and only while they are current.
  Writing a note queues a sweep too, since nothing else would.
  A drawer is read once: changing the provider later does not re-read what was already read.
- A drawer's first 8,000 characters are sent, whatever it holds.
- Every fact records the drawer, source document, job and extractor it came from, and holds from the document's own
  date when the source has one.
  When a re-mine replaces a drawer, the facts it supported stop being current and the new drawer is read.

## Deduplication

The `[dedup]` section controls how eagerly MemCastle recognises the same memory, or the same entity, written twice.
It never merges anything: the most it does is not store an exact copy, link a likely copy to what it resembles,
and let an entity spelled differently converge on the one it is a variant of.
The rules, and what each writer gets back, are on the [Deduplication](deduplication.md) page.

- `enabled = false` stores every drawer and records no link.
  Entity names still converge when they differ only in case, punctuation or spacing, which is spelling and not a
  judgement.
- `near_threshold` is the similarity at which a drawer is linked as a `near` duplicate of another in its room.
  Raise it towards `1` to link only the closest copies; below `0.5` it would link unrelated drawers, so it is refused.
  It never makes MemCastle skip a write: only an exact copy does.
- `entity_fuzzy = false` leaves casing, punctuation and alias convergence on,
  and turns off the one-character typo rule and the `possibly_same_as` candidates.

## The database admin endpoint

`memcastle db start` opens a second, SurrealDB-compatible listener inside the running daemon, so SurrealDB Studio can
inspect the live embedded database.
The `[db]` settings are only the defaults for that command; no setting starts the endpoint.
Each flag of `memcastle db start` (`--bind`, `--port`, `--allow-remote`, `--allow-origin`) overrides its setting,
and `--allow-origin` adds to `db.allowed_origins`.

The address is `127.0.0.1` port `8000` unless configured otherwise, and the port must differ from `server.port`.
A `db.bind` that is not a loopback address is refused when the configuration loads unless `db.allow_remote` is true,
and the daemon refuses to open it at all unless `auth.enabled` is also true.
`db.allowed_origins` lists web page origins, such as `https://app.surrealdb.com`, that may connect from a browser,
in addition to pages served from this machine and the SurrealDB Studio desktop app, which are always allowed.
The values are compared whole, never as patterns.

See [Database access](database-access.md) for the workflow and the security model.

## Triggers

A trigger asks for a miner's run on its own: on a timetable, on a poll, when a file changes, or when a webhook is
called.
Triggers are the `[[triggers]]` entries of the configuration file, managed from the file or from
[`memcastle trigger`](cli.md#trigger) ([ADR-043](adr/043-source-triggers.md)).
**Every trigger is disabled until `enabled = true`**, and nothing the daemon does ever sets it.

```toml
[[triggers]]
name = "notes-watch"       # unique: lowercase letters, digits, `-` and `_`; `reload` is reserved
miner = "notes"            # the miner it asks to run
type = "watch"             # "schedule", "poll", "webhook" or "watch"
enabled = false            # the default
path = "/home/alice/notes" # the other keys depend on the type
debounce = "2s"

[[triggers]]
name = "notes-hook"
miner = "notes"
type = "webhook"
credential = { type = "env", name = "NOTES_HOOK_SECRET" }   # or { type = "file", path = "/run/secrets/hook" }
delivery_header = "x-delivery-id"
```

The settings of each type, what enabling needs, the webhook listener (`[webhook]` above), its security and
networking prerequisites, bursts, repeats, restarts and failures are on the [Triggers](triggers.md) page.
`[webhook]` is read when the daemon starts, and a `[[triggers]]` entry is read again whenever the file changes.

## Runtime assets

Some files a release carries are not your configuration or your data.
They are read-only, come with the version of MemCastle you installed, and are found by one fixed rule, tried in order:

1. **An explicit directory**, from `--assets-dir`, `MEMCASTLE_ASSETS_DIR` or `assets.dir`, by the usual precedence.
   It must be an absolute path to an existing directory.
   If it does not exist the daemon refuses to start with `memcastle::assets::not_found`;
   it never quietly uses another source in its place.
2. **The installed directory**, which an OS package creates: `share/memcastle` next to the executable's `bin/`
   (`/usr/share/memcastle` for `/usr/bin/memcastle`), then `/usr/local/share/memcastle`, then `/usr/share/memcastle`.
   The first that exists is used.
   A candidate inside your XDG data directory is ignored, because that is where your palace lives.
3. **The assets built into the binary.**
   This is what a standalone download uses, and it needs no directory and no network.

The daemon logs which source it resolved at startup (`runtime assets resolved`).
The schema and the data migrations are always built into the binary and are not affected by any of this.
[Installation](installation.md#standalone-binary-or-native-package) describes the package layout.

This is the one root for everything a package ships, in the same layout wherever it is:

| Under the root | What | Read by |
|---|---|---|
| `sources/<name>/` | the bundled mining sources, unpacked: a `memcastle-source.toml` and a `source.wasm` each | the daemon, which lists them as installed and runs them in place (see [Publishing and installing sources](publishing-sources.md#bundled-sources)) |
| `skills/` | the shared agent skills | `memcastle integration`, which copies the ones an integration's manifest names into its installed copy |
| `web/dist/` | the built [web UI](web.md) | the daemon, under `/ui`, when `web.enable` is set |

A standalone binary has none (the dashboard is not built into it, so with `web.enable` it answers a page that says the
files are missing), so point the setting at a package or checkout only when it supplies assets you need.
The bundled sources are taken from `<root>/sources` when a directory in it holds a `memcastle-source.toml` and a
`source.wasm`, and `mining.bundled_dir` outranks that.
A root with no such package, like a checkout (whose `sources/` holds projects, with their component under `dist/`),
leaves the sources to the installed assets.
`memcastle integration` reads the root from the same three places as the daemon; `--assets-dir` is accepted by its
`list`, `install` and `update` commands.

## Miners

### Source configurations

A source configuration names reusable source-native defaults and a credential reference; it has no destination or schedule.
The default ID is the source ID (`chatgpt` for `chatgpt`), even when nothing is written in TOML.
Additional configurations live in the global file and have unique IDs:

```toml
[[source_configs]]
id = "github-work"
source = "github"
credential = { type = "env", name = "GH_TOKEN" }

[source_configs.options]
include = ["my-team/*"]

[[miners]]
name = "work-wiki"
source = "github"
source_config = "github-work"
wing = "work"

[miners.options]
wiki_include = ["my-team/wiki"]
```

`memcastle source config list|get|set|remove` and authenticated `/api/source-configs` manage this section.
MCP can list configuration summaries but cannot edit or authenticate them.
A miner selects its source's default configuration when `source_config` is absent.
Source configuration options are applied first and matching miner options replace them;
one-off `miner run` options replace matching saved options without rewriting either definition.
A change to shared source defaults that may read more material needs `--allow-broaden` in the CLI
or `allow_broaden = true` in the REST body; the UI asks for the same explicit choice.
The common `mining.max_documents` default (zero for unlimited) is overridden by a miner's positive `max_documents`.
`embedding_profile` selects a runnable named embedding profile for drawers this miner files.
The resolved vector space is pinned to its durable job; an explicit embedding-job profile outranks the miner override,
which outranks the global default.
Credentials and their paths are never returned by a configuration view; OAuth sign-ins for the default configuration
retain the existing source-keyed credential so upgrades do not lose them.
ChatGPT's current web authorization can select only the default account: an additional named ChatGPT web configuration
cannot authenticate yet; export-import miners remain usable without an account.
Legacy miner `credential` references still load, but put new reusable credentials on the source configuration.

For a source named `team-feed` whose manifest declares `needs_credentials = true`,
`[permissions] credential = true` and the network permission it needs to fetch its feed,
two accounts can use separate references without putting either value in TOML:

```toml
[[source_configs]]
id = "team-feed-work"
source = "team-feed"
credential = { type = "env", name = "TEAM_FEED_WORK_TOKEN" }

[[source_configs]]
id = "team-feed-personal"
source = "team-feed"
credential = { type = "file", path = "/run/secrets/team-feed-personal" }

[[miners]]
name = "work-daily"
source = "team-feed"
source_config = "team-feed-work"
wing = "work"

[[miners]]
name = "work-weekly"
source = "team-feed"
source_config = "team-feed-work"
wing = "work"

[[miners]]
name = "personal-daily"
source = "team-feed"
source_config = "team-feed-personal"
wing = "personal"
```

Start the daemon with `TEAM_FEED_WORK_TOKEN` set and put the personal token on the file's first line.
The two work miners reuse one configuration; the personal account and destination have separate source records and
cursors.
Changing a configuration's reference changes future jobs only; a queued or resumed job keeps its selected reference
but reads its current value again when it runs.
Existing miner-level `credential = { type = "env", name = "..." }` remains a legacy way to select a reference.
Installation preview and consent name the selected-credential permission, and an update adding it requires fresh consent.

### Miner definitions

A miner is a named, persistent definition of what to mine and how: a source or reusable source configuration,
input locator, options, destination, per-job limit and optional schedule.
Definitions live in global `[[miners]]` entries or one file per miner under `$XDG_CONFIG_HOME/memcastle/miners/`.
An explicit `--config` in another directory keeps new miners in that global file and does not load unrelated XDG miners.
Additional directories can be listed under `[mining]` as `miner_dirs = ["/path/to/team/miners"]`.
Files are read in filename order, after the global section and default directory.
A file contains the same fields as a `[[miners]]` entry, at its top level rather than inside a table.
For example, `~/.config/memcastle/miners/ChatGPT Dev.toml` may contain:

```toml
source = "chatgpt"
locator = "/exports/latest.zip"
wing = "personal"
room = "conversations"
max_documents = 300

[schedule]
every = "1d"
at = "03:30"
enabled = false
```

When `name` is absent its filename stem is trimmed, lowercased and spaces become hyphens:
`ChatGPT Dev.toml` defines `chatgpt-dev`, invoked with `memcastle mine chatgpt-dev`.
An explicit `name` takes precedence, and duplicate IDs are reported with both paths rather than silently overridden.
Existing definitions are edited at their originating file; files are checked for concurrent edits and replaced atomically.
They are managed from the file, from [`memcastle miner`](cli.md#miner) and, read-only, from MCP
([ADR-037](adr/037-persistent-miner-configuration.md)).

```toml
[[miners]]
name = "signal-personal"          # unique: lowercase letters, digits, `-` and `_`; `reload` is reserved
source = "signal"                 # the source adapter: `directory`, or an installed source's name
enabled = true                    # default true
locator = "+336..."               # where in the source to read; the source decides what it means
wing = "signal"                   # the wing its drawers go to, when it should not be the source's default
room = "conversations"            # optional fixed room
max_documents = 500               # overrides the common per-job cap; omit to inherit it
mapping = { wing_from = "project_name", room_from = "conversation_group" } # optional source-independent metadata routing
credential = { type = "env", name = "SIGNAL_TOKEN" }   # or { type = "file", path = "/run/secrets/signal" }, or { type = "oauth" }

[miners.options]                  # source options: strings, numbers, booleans, lists of strings
contacts = ["+336..."]
groups = ["MemCastle"]
window_days = 30
```

A key that is not listed is an error, not an ignored typo: `enable = false` must not leave a miner enabled.
`options` is an open table, so a source can define keys of its own.
Legacy `[miners.scope]` and `[miners.config]` tables must be merged by hand into `[miners.options]` before starting
this version; keys present in both need one chosen value.
Likewise REST callers must replace `scope` and `config` with `options`, and `unset_scope` and `unset_config` with
`unset_options`.

- **Identity.** A miner's cursor and documents belong to the *source* it points at, and not to its name.
  Renaming, enabling, disabling or changing a credential keeps its cursor
  when the acquisition scope and mapping stay the same.
  Pointing it at another source or locator creates a different source with its own cursor.
  A source such as `chatgpt` can also give different selected project scopes separate identities and cursors.
  Removing a miner, or disabling it, never deletes what it mined.
  A configured Wing, Room or named source configuration gives a different destination
  an independent cursor and document set.
  A `mapping.wing_from` or `mapping.room_from` key reads a string from each document's metadata.
  Missing or invalid names fall back to the fixed `wing`/`room` or source defaults.
  Different mapping definitions have independent cursor and document records, so two miners can reuse one sign-in
  without treating a differently filed conversation as already ingested.
- **Secrets.** A secret is never written in the file.
  `credential` names where it comes from: an environment variable of the daemon, a file,
  or `{ type = "oauth" }` for a source that signs in with OAuth (see [Credentials](#credentials)), which names nothing
  because the daemon keeps the tokens itself.
  A key in `options` that reads like a secret (`password`, `token`, `secret`, `api_key`, ...)
  is refused, and the daemon reports a credential only as `env`, `file` or `oauth` and whether it resolves
  (for `oauth`, whether the source is signed in), never its name or path.
  A source with consented `[permissions] credential = true` can request the selected `env` or `file` value during
  `identify`, `discover` and `read`; `normalize` and host-run processes never receive it implicitly.
  An enabled miner whose variable is unset, or whose file's first line is unreadable, invalid UTF-8 or empty,
  is `unavailable` with the fix.
  An OAuth sign-in is handed over: a source that declares one is given a fresh access token on each call that asks.
  Sources requiring it for every mode make a miner unavailable until `memcastle source auth <source>` has been run;
  a source with on-demand OAuth may also offer an offline mode or an explicit manual-session fallback.
- **Validation.** The shape of every entry is checked when the file is loaded, so a bad entry stops the daemon starting
  like a bad `[embeddings]` section.
  An *enabled* miner is also held to what running needs, when it is created or changed: the source is built in or
  installed and enabled, the credential resolves, every `options` key is one the source declares as an option,
  and for `directory` the `locator` is an absolute path.
  A disabled miner may name a source that is not installed yet, so a configuration can be written ahead of the install.
- **Schedule.** A miner runs when asked with `memcastle miner run <name>` or `memcastle mine <name>`.
  Add `[miners.schedule] every = "1d"` and optionally `at = "03:30"` (UTC), or `[schedule]` in a miner file.
  The timetable remains disabled until `memcastle miner schedule-enable <name>` (or REST/UI) explicitly enables it.
  The daemon reconciles it through the same trigger supervisor as [standalone triggers](#triggers);
  removal, a disabled miner or a disabled schedule stops the task without discarding mined data.
- **Options.** The `options` table holds saved source options: `miner run` passes them to the source
  exactly as `memcastle mine <source> key=value` would ([ADR-042](adr/042-mine-takes-a-source-and-its-options.md)).
  A scalar is its text and a list of strings is comma-joined; an undeclared key or nested table makes the miner not
  runnable, so the daemon never silently ignores an option.
  `miner run NAME key=value` replaces or adds an option for that run only and leaves the saved definition unchanged.
  A change that may include more material, whether persistent or temporary, requires `--allow-broaden`.
  Sources can declare include/exclude/date comparisons; an option without declared comparison semantics requires
  approval whenever its value changes.
  `directory` declares `since`, `include` and `exclude` (comma-separated relative-path globs);
  `pi`, `opencode` and `claude` declare `since` and `dir`.
  `chatgpt` declares a web-only `projects` filter in addition to `mode` and `account` options.
  The external `memcastle-github` plugin's source requires `include` repository patterns; `exclude` and separate wiki
  patterns can narrow them further.

### Claude Code history miner

The externally installed `claude` source reads only session transcripts under `~/.claude/projects`.
Define a miner explicitly; no Claude Code client or source installation creates one:

```toml
[[miners]]
name = "claude-history"
source = "claude"
locator = "/home/alice/.claude/projects"
enabled = true

[miners.options]
since = "2026-09"
dir = "/home/alice/src/*"
```

`since` narrows by transcript modification time and `dir` matches the working directory recorded in each transcript.
Use a separate miner or run `memcastle mine claude --full` when a restored transcript predates its cursor.

### ChatGPT project miner

The external `chatgpt` source can select projects on its experimental web backend using miner options.
Sign the source in with `memcastle source auth chatgpt` on the daemon's machine before running web miners.
This source's desktop-compatible OAuth bearer was verified to read history without cookies using a browser-style
User-Agent; the old manually supplied web session remains a fallback ([ChatGPT](mining-sources.md#chatgpt)).

```toml
[[miners]]
name = "chatgpt-work"
source = "chatgpt"

[miners.options]
projects = ["name:Work", "id:g-p-example"]
mode = "web"
```

The miner joins the `projects` list with commas and the source resolves names to unique project IDs.
If a name no longer exists or names collide, the run fails rather than broadening its scope.
Use project IDs for scheduled miners: names can be reused by another project after a rename.
Without `projects`, a web miner includes ordinary conversations and every project's history; exports do not support
project selection.
Changing or removing an option that broadens project selection still follows the `--allow-broaden` rule.

### GitHub repository and wiki miners

The `memcastle-github` plugin's source requires an explicit repository `include` pattern.
`org/*` selects repositories owned by an organization; an `exclude` pattern overrides inclusion.
The wiki can be selected independently within those repositories with `wiki_include` and `wiki_exclude`.

```toml
[[miners]]
name = "acme-github"
source = "github"
credential = { type = "env", name = "GH_TOKEN" }

[miners.options]
include = ["acme/*"]
exclude = ["acme/private-experiment"]
wiki_include = ["acme/docs"]
comments = true
reviews = true
since = "2026-01-01"
```

Set `GH_TOKEN` or `GITHUB_TOKEN` in the daemon's environment; when both are set, `GH_TOKEN` wins.
The miner's env credential reference checks that variable exists, while the source's explicit manifest grant actually
delivers it; choose the same name in both places.
Omit `credential` for public repositories if unauthenticated GitHub rate limits suffice.
For a wiki-only miner, set `issues = false`, `pulls = false` and `metadata = false` under `[miners.options]` and define
`wiki_include` there too.
CLI `memcastle miner set` accepts the same options with repeatable `--option 'include=acme/*'` and
`--option 'exclude=acme/private-experiment'` flags, and the REST miner API accepts the same table.
MCP can list/get this miner but cannot change it.
See [GitHub source](mining-sources.md#github) for Git prerequisites, path/label/topic filters and cursor behavior.

A miner can override the global embedding profile with `embedding_profile = "bulk"` as a top-level `[[miners]]` field.
This is not a source acquisition option: it does not go to the source adapter or alter its cursor.
The miner pins its resolved profile on the mine job and queues a separate, pinned, missing-only embedding job limited
to that mined source when mining finishes; a `wing` further restricts it if set.
An explicit `memcastle embed --profile NAME` selects its own profile instead.
Set it through `memcastle miner set NAME --embedding-profile bulk --wing WING` or unset it with
`memcastle miner set NAME --unset embedding_profile`.

### Changing miners while the daemon runs

The configured global file and individual miner files are the source of truth, and the daemon is their only writer.
`miner set`, `enable`, `disable` and `remove` edit only the `[[miners]]` section, in place:
comments, the other tables and every entry that did not change are left exactly as they were,
and the file is replaced atomically with its permissions kept.
The environment and command-line overrides are not part of the file, so they are never written into it.

- **Hand edits are picked up.**
  The daemon notices that the file changed on the next request about miners, and `memcastle miner reload` reads it at
  once and says what differs: `added`, `removed`, `changed`, `enabled`, `disabled` and `broadened`.
  A miner is never half-applied: each entry is whole or the file is refused.
- **An invalid edit does not take the miners away.**
  The daemon keeps the last miners that were valid, reports the error beside them (`miner list`, `GET /api/miners`),
  and refuses to change miners until the file is fixed, because writing would overwrite what is being repaired.
  A file that is invalid at startup stops the daemon, like the rest of the configuration.
- **Reload reconciles enabled schedules.**
  A miner with no explicitly enabled schedule remains manual; a configured timetable starts, changes or stops idempotently.
  A job that is already queued or running when a miner is disabled or removed finishes,
  because a job carries the source and locator and not the miner's name.
- **Option changes never broaden silently.**
  `miner set` refuses a change it cannot prove does not read more, with the existing
  `memcastle::miner::scope_broadened` diagnostic, unless `--allow-broaden` says it is meant.
  Positive selections narrow when values are removed; exclusions narrow when values are added; a later `since` narrows.
  Unknown semantics require approval for changes.
  A hand edit that may widen a miner is applied, since the file is yours, but the daemon logs it at `warn`
  and `miner reload` lists it under `broadened`.
- **A daemon started without a configuration file** (one built in code, as in a test) lists miners but refuses to
  change them: there is nowhere to keep them.
  `memcastle` always knows the file: `--config`, `MEMCASTLE_CONFIG`, or `~/.config/memcastle/config.toml`,
  which is created by the first `miner set`.

## Credentials

A mining source that cannot be reached with a static token declares an OAuth sign-in in its manifest,
and `memcastle source auth <source>` signs it in ([ADR-039](adr/039-oauth-credentials-for-mining-sources.md)).
The daemon keeps what that produces, renews it when a run needs it, and hands the source an access token for each call;
the source never owns a store of its own, and the tokens are never in this file, the palace database, the logs, or
anything a REST or MCP answer returns.

`credentials.backend` chooses where the tokens are kept:

- `auto` (the default) uses the platform's credential store when one works: the Keychain on macOS, the Credential Manager
  on Windows, the Secret Service on Linux (GNOME Keyring, KWallet and the like).
  It is probed on first use and not at startup, since probing can prompt for a keychain password,
  and when it does not work (a Linux server with no desktop session, a container) the tokens go to an owner-only file instead.
  A credential is looked for in both, so one written to the file while the keyring was down is still found when it is back.
- `keyring` uses the platform store only, and signing in fails where there is none.
- `file` uses `credentials.dir` only: one `<source>.json` per source, the directory `0700` and each file `0600` on Unix,
  written to a temporary file and renamed so that a crash never leaves half a token.
  It is the setting for a headless host: the files are exactly as private as the account that runs the daemon.
  The directory defaults to `$XDG_DATA_HOME/memcastle/credentials`, and must be absolute.

What is kept is the refresh token, which is what lets a source stay signed in for months,
with the scopes granted and a fingerprint of the client, endpoints and scopes the manifest declared at the time.
The short-lived access token is held in the daemon's memory, and is written down only for a provider that issues no
refresh token, until it expires.
If an update to a source changes its client, its endpoints or its scopes, the stored credential no longer matches,
counts as missing, and the source asks to be signed in again under the new terms.
A credential is shared by every palace on the machine that uses the same keyring or directory, since it belongs to the
installed source and not to a palace.

## Web UI

`web.enable = true` (or `MEMCASTLE_WEB_ENABLE=true`) makes the daemon serve its web UI under `/ui`.
It is off by default: a daemon that does not set it answers nothing at `/ui`.
There is no command-line flag; `daemon start` and `daemon restart` pass the environment and the file to the daemon as usual.

The dashboard's files are found in `web/dist/` of the [runtime assets](#runtime-assets),
in a package and in a checkout alike.
If they are missing, the daemon still starts, logs a warning, and answers `/ui` with a page that says how to install them.
See [Web UI](web.md) for the login, the memory mode and the development workflow.

## Not supported: other platforms' conventions

Windows is built and tested, but has no dedicated layout: the same dot-directories are used under your home directory.
Any platform-specific behaviour would be an explicit design decision, recorded in an ADR;
see [ADR-010](adr/010-unix-xdg-paths.md).

## See also

- [Running the daemon](daemon.md) for starting, stopping and supervising it.
- [Storage and data](storage.md) for what lives in the palace and the registry.
- [CLI reference](cli.md) for every flag.
- [Troubleshooting](troubleshooting.md) for configuration errors.
