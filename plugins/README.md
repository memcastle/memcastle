# In-tree provider plugins

`plugins/example` is the in-tree fixture for plugin packaging and tests.
Its source and integration modules remain under `modules/<stable-module-id>/` with their own manifests and state.
Core CI checks this fixture explicitly on every pull request and main push; providers live in standalone repositories
with their own CI and release workflows.

The same project can move into its own repository without changing `plugin.id`, module IDs, source cursors or data.
The standalone project keeps `scripts/package.py` and its own `.github/workflows/` from the GitTPL parent template.
The legacy `sources/`, `integrations/` and `docs/registry.json` continue to serve v0.4 consumers during migration.
