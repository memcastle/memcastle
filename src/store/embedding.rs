//! Per-space vectors, separate from immutable drawer and graph records.

use std::collections::HashMap;

use serde::Deserialize;

use crate::domain::{Drawer, DrawerId, EmbeddingSpace, SourceId};
use crate::error::Result;

use super::SurrealStore;
use super::drawers::DRAWER_SEARCH_COLUMNS;

/// Only these schema-declared dimensions have a dedicated index slot.
fn indexed_table(dimensions: usize) -> Option<&'static str> {
    match dimensions {
        768 => Some("embedding_fast_768"),
        1536 => Some("embedding_fast_1536"),
        3072 => Some("embedding_fast_3072"),
        _ => None,
    }
}

#[derive(Deserialize)]
struct Slot {
    dimension: i64,
    space: String,
}

#[derive(Deserialize)]
struct ExistingEmbedding {
    content_hash: String,
    input_hash: String,
}

#[derive(Deserialize)]
struct PageEmbedding {
    drawer: String,
    content_hash: String,
    input_hash: String,
}

impl SurrealStore {
    /// A tier belongs to the first space that claims it, permanently; later profiles take the exact path.
    pub(crate) async fn indexed_embedding_table(
        &self,
        fingerprint: &str,
        dimensions: usize,
    ) -> Result<Option<&'static str>> {
        let Some(table) = indexed_table(dimensions) else {
            return Ok(None);
        };
        let mut response = self.db.query("SELECT dimension, space FROM embedding_slot WHERE id = type::record('embedding_slot', $tier)")
            .bind(("tier", dimensions.to_string())).await?;
        let rows: Vec<Slot> = super::take_rows(&mut response, 0)?;
        Ok(rows
            .first()
            .filter(|slot| slot.space == fingerprint && slot.dimension == dimensions as i64)
            .map(|_| table))
    }

    async fn claim_embedding_tier(&self, space: &EmbeddingSpace) -> Result<Option<&'static str>> {
        let Some(table) = indexed_table(space.dimensions) else {
            return Ok(None);
        };
        if let Some(assigned) = self
            .indexed_embedding_table(&space.fingerprint, space.dimensions)
            .await?
        {
            return Ok(Some(assigned));
        }
        // CREATE ONLY is atomic across daemons; a competing profile may win, in which case we use exact ranking.
        let _ = self.db.query("CREATE ONLY type::record('embedding_slot', $tier) SET dimension = $dimension, space = $space")
            .bind(("tier", space.dimensions.to_string())).bind(("dimension", space.dimensions as i64))
            .bind(("space", space.fingerprint.clone())).await;
        let assigned = self
            .indexed_embedding_table(&space.fingerprint, space.dimensions)
            .await?;
        Ok(assigned.and(Some(table)))
    }
    /// Read a vector from precisely one space, useful for verifying writes without reading legacy data.
    pub async fn get_profile_embedding(
        &self,
        id: DrawerId,
        space: &str,
    ) -> Result<Option<Vec<f32>>> {
        #[derive(Deserialize)]
        struct Row {
            vector: Vec<f32>,
        }
        let mut response = self.db.query("SELECT vector FROM drawer_embedding WHERE drawer = $drawer AND space = $space LIMIT 1")
            .bind(("drawer", id.to_string())).bind(("space", space.to_string())).await?;
        let rows: Vec<Row> = super::take_rows(&mut response, 0)?;
        Ok(rows.into_iter().next().map(|row| row.vector))
    }
    /// Return only the derived hashes, not the vector, for a resumable sweep.
    pub async fn embedding_hashes(
        &self,
        id: DrawerId,
        space: &str,
    ) -> Result<Option<(String, String)>> {
        let mut response = self.db.query("SELECT content_hash, input_hash FROM drawer_embedding WHERE drawer = $drawer AND space = $space LIMIT 1")
            .bind(("drawer", id.to_string())).bind(("space", space.to_string())).await?;
        let rows: Vec<ExistingEmbedding> = super::take_rows(&mut response, 0)?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| (row.content_hash, row.input_hash)))
    }

    /// Read only the hashes for one bounded drawer page and exactly one space in a single query.
    pub async fn embedding_hashes_for_page(
        &self,
        drawers: &[Drawer],
        space: &str,
    ) -> Result<HashMap<String, (String, String)>> {
        if drawers.is_empty() {
            return Ok(HashMap::new());
        }
        let ids: Vec<String> = drawers.iter().map(|drawer| drawer.id.to_string()).collect();
        let mut response = self.db.query("SELECT drawer, content_hash, input_hash FROM drawer_embedding WHERE space = $space AND drawer IN $ids")
            .bind(("space", space.to_string())).bind(("ids", ids)).await?;
        let rows: Vec<PageEmbedding> = super::take_rows(&mut response, 0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.drawer, (row.content_hash, row.input_hash)))
            .collect())
    }

    /// A bounded page in stable id order. Restarting at the first page is safe: writes are idempotent.
    pub async fn embedding_drawer_page(
        &self,
        wing: Option<&str>,
        source_id: Option<SourceId>,
        after: &str,
        limit: u32,
    ) -> Result<Vec<Drawer>> {
        let sql = format!(
            "SELECT {DRAWER_SEARCH_COLUMNS} FROM drawer WHERE record::id(id) > $after \
             AND ($source_id = NULL OR source.origin.source_id = $source_id) \
             AND ($wing = NULL OR room IN (SELECT VALUE record::id(id) FROM room WHERE wing IN \
             (SELECT VALUE record::id(id) FROM wing WHERE name = $wing))) ORDER BY id ASC LIMIT $limit"
        );
        let mut response = self
            .db
            .query(sql)
            .bind(("after", after.to_string()))
            .bind(("wing", super::bindable(&wing)?))
            .bind((
                "source_id",
                super::bindable(&source_id.map(|id| id.to_string()))?,
            ))
            .bind(("limit", limit))
            .await?;
        super::take_rows(&mut response, 0)
    }

    /// Write only if the content read for generation is still present.
    /// The record id is deterministic so replay updates the same derived row.
    pub async fn set_profile_embedding(
        &self,
        drawer: &Drawer,
        space: &EmbeddingSpace,
        input_hash: &str,
        vector: &[f32],
    ) -> Result<bool> {
        if vector.len() != space.dimensions {
            return Err(crate::error::Error::EmbeddingDimension {
                expected: space.dimensions,
                actual: vector.len(),
            });
        }
        if vector.iter().any(|x| !x.is_finite()) {
            return Err(crate::error::Error::EmbeddingFailed {
                message: "the embedding contains a non-finite number".into(),
            });
        }
        let key =
            crate::domain::sha256_hex(format!("{}:{}", drawer.id, space.fingerprint).as_bytes());
        let tier = self.claim_embedding_tier(space).await?;
        // The table name comes only from the closed set above, never from a caller or config.
        let projection = tier.map_or(String::new(), |table| {
            format!("UPSERT type::record('{table}', $key) SET drawer = $drawer, vector = $vector;")
        });
        let sql = format!(
            "BEGIN TRANSACTION; \
                 LET $current = (SELECT VALUE content_hash FROM drawer WHERE id = type::record('drawer', $drawer)); \
                 IF array::len($current) = 1 AND $current[0] = $content_hash {{ \
                   UPSERT type::record('drawer_embedding', $key) SET drawer = $drawer, space = $space, \
                     profile = $profile, provider = $provider, provider_type = $provider_type, \
                     connection = $connection, model = $model, dimensions = $dimensions, \
                     pipeline_version = $pipeline_version, input_hash = $input_hash, \
                     content_hash = $content_hash, vector = $vector, created_at = time::now(); \
                   {projection} \
                 }}; \
                 COMMIT TRANSACTION; \
                 RETURN $current;"
        );
        // Guard both existence and the content version before touching the derived record.
        let mut response = super::retrying_on_conflict(|| async {
            super::checked(
                self.db
                    .query(sql.clone())
                    .bind(("drawer", drawer.id.to_string()))
                    .bind(("key", key.clone()))
                    .bind(("content_hash", drawer.content_hash.clone()))
                    .bind(("space", space.fingerprint.clone()))
                    .bind(("profile", space.profile.clone()))
                    .bind(("provider", space.provider.clone()))
                    .bind(("provider_type", space.provider_type.clone()))
                    .bind(("connection", space.connection.clone()))
                    .bind(("model", space.model.clone()))
                    .bind(("dimensions", space.dimensions as i64))
                    .bind(("pipeline_version", i64::from(space.pipeline_version)))
                    .bind(("input_hash", input_hash.to_string()))
                    .bind(("vector", vector.to_vec()))
                    .await?,
            )
        })
        .await?;
        let rows: Vec<String> = super::take_rows(&mut response, 4)?;
        Ok(rows
            .first()
            .is_some_and(|hash| hash == &drawer.content_hash))
    }
}
