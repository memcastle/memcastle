//! An explicit watermark for the change from unverified legacy vectors to named spaces.

use super::MigrationFuture;
use crate::store::SurrealStore;

/// Preserve provenance-free `drawer.embedding` values for recovery.
/// All profile-aware search uses `drawer_embedding`, so no data rewrite or model guess is safe here.
pub(super) fn apply(store: &SurrealStore) -> MigrationFuture<'_> {
    Box::pin(async { store.retire_legacy_embedding_index().await })
}
