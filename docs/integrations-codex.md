# Codex integration

Install the [`memcastle-openai`](https://github.com/memcastle/memcastle-openai) plugin first, after reviewing the
`codex` and `chatgpt` source consent digests shown by its preview.
`memcastle integration install codex` then registers its Codex integration through a local marketplace.
Already installed Codex integrations retain their agent receipt; run `memcastle integration install codex` after installing
the external plugin to refresh the installed copy when its assets differ.
It uses Codex's native HTTP MCP transport at `http://127.0.0.1:8420/mcp`.
When the daemon requires authentication, export `MEMCASTLE_AUTH_TOKEN` before starting Codex.
The installer never edits `~/.codex/config.toml` and never writes a token.

The plugin exposes the shared MemCastle skills and the MCP tools supplied by the daemon.
Use the `checkpoint-instructions` skill and `memcastle_checkpoint` to save explicitly classified items.

Codex does not expose a supported plugin API for session startup, transcript access, context compaction, or a plugin-owned
long-lived MCP connection.
Consequently wake-up injection, automatic and emergency checkpoints, reconnect handling, and automatic per-session
memory-mode selection are unavailable.
Use the shared skills, explicit MCP calls, and daemon-side triggers instead.
