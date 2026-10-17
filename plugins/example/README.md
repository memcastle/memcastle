# example

A MemCastle provider plugin.

This provider plugin owns the versioned package, while each module has its own lifecycle.
Installing the plugin makes its modules available; enable sources and install integrations separately.

```sh
python3 scripts/package.py
```

The archive in `dist/` is independently releasable from MemCastle Core.
The immutable `plugin.toml` declares stable module identities; packaging regenerates `manifest.toml` with the
release version and checksums of the built artifacts and writes `dist/example-0.1.0.sha256sum` for the archive.
Commit `manifest.toml` and each source module's `Cargo.lock` before publishing a release.
Keep IDs stable when moving this project from `plugins/example/` to its own repository.
