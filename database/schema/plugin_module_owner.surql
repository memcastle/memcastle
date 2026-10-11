-- An installed module ID is claimed transactionally by one plugin; unlike historical source identity,
-- integration and metadata-only embedding IDs may be reused after the providing plugin is removed.
DEFINE TABLE IF NOT EXISTS plugin_module_owner SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON TABLE plugin_module_owner TYPE string;
DEFINE FIELD IF NOT EXISTS plugin ON TABLE plugin_module_owner TYPE string;
