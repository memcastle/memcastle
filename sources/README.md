# Reference sources

One directory per source, each a complete source package (docs/writing-sources.md): a `memcastle-source.toml`, the code
that implements `wit/memcastle-source.wit`, and the conformance cases it is tested with.

| Directory | Source name | What it is |
|---|---|---|
| `directory/` | `directory-wasm` | The built-in `directory` source, written as a Rust component. The worked example. |

They are maintained as examples and as the second implementation the conformance cases are run against, so that the
built-in and the WebAssembly contract cannot drift apart.
A source here is built with `memcastle source build` and tested with `memcastle source test`;
`mise run sources:check` builds and tests them all, and `mise run sources:test -- <name>` one.
A source that wraps a program ships a stand-in for it in `fixtures/bin/`, which the two tasks put first
on the `PATH`; run `memcastle source test` by hand with the same prefix.
Core CI explicitly tests `directory`; adding another reference source to CI requires a workflow matrix entry.
They are not compiled into MemCastle, and a source that ships with MemCastle's releases uses exactly the same package
contract as one a user installs.
`packaging/sources/build.sh` writes an empty bundle until Core ships another source;
`mise run sources:package` verifies that layout locally.
`docs/publishing-sources.md` describes how a source reaches users through a provider plugin.
OpenCode history is maintained with its agent integration in the external
[`memcastle-opencode`](https://github.com/memcastle/memcastle-opencode) plugin.
The GitHub repository source lives in the [`memcastle-github`](https://github.com/memcastle/memcastle-github) plugin.
Claude Code history and its agent integration live in
[`memcastle-anthropic`](https://github.com/memcastle/memcastle-anthropic).
Codex and ChatGPT history and the Codex integration are maintained in the external
[`memcastle-openai`](https://github.com/memcastle/memcastle-openai) plugin.
