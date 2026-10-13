# In-tree provider plugins

Each directory under `plugins/` with a root `plugin.toml` is one independently versioned release unit.
Its source and integration modules remain under `modules/<stable-module-id>/` with their own manifests and state.
Plugin CI discovers every directory with a root `plugin.toml` and checks all providers on every pull request and main push.

The same directory can move into its own repository without changing `plugin.id`, module IDs, source cursors or data.
The standalone project keeps `scripts/package.py` and its own `.github/workflows/` from the GitTPL parent template.
The legacy `sources/`, `integrations/` and `docs/registry.json` continue to serve v0.4 consumers during migration.
