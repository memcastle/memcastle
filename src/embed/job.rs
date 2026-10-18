//! The embed job handler: populate or explicitly refresh one immutable vector space.
//!
//! Each pass scans a bounded, stable drawer page and persists its last id.
//! On recovery the job uses its pinned profile and resumes after the last
//! completed page; missing-only sweeps skip vectors already in that space.
//! A refresh only changes derived records, never drawer content or timestamps.
//!
//! A provider failure fails the job rather than skipping the drawer: silently
//! leaving holes would make semantic search quietly worse, while a failed job
//! is visible in `memcastle job list`. The scheduler's retry budget and the
//! next sweep cover a provider that comes back.

use crate::domain::{EmbeddingRefresh, EmbeddingSpace, Job, JobProgress, SourceId};
use crate::error::{Error, Result};
use crate::jobs::{JobContext, JobOutcome};

/// How many drawers one pass reads and embeds. Bounds memory (a drawer can
/// hold 256 KiB) and how much work a stop request waits behind.
const PASS_SIZE: u32 = 32;

/// What an `Embed` job needs, gathered from its [`crate::domain::JobKind`].
pub struct EmbedParams {
    /// Restrict the sweep to one wing by name.
    pub wing: Option<String>,
    /// Restrict a miner-driven sweep to its source's drawers, even when the wing is shared.
    pub source_id: Option<SourceId>,
    /// Named profile selected at submission.
    pub profile: Option<String>,
    /// Whether existing vectors are eligible for replacement.
    pub refresh: EmbeddingRefresh,
    /// Pinned non-secret identity; an old unpinned job is rejected.
    pub space: Option<EmbeddingSpace>,
}

/// Embed drawers until none are left without a vector, or the job is stopped.
///
/// # Errors
///
/// [`Error::EmbeddingsNotConfigured`] without a provider, [`Error::EmbeddingFailed`]
/// when it fails, or a store error.
pub async fn run(ctx: &JobContext, job: &mut Job, params: EmbedParams) -> Result<JobOutcome> {
    let EmbedParams {
        wing,
        source_id,
        profile,
        refresh,
        space,
    } = params;
    // Old queued jobs had no pinned pipeline; guessing after a restart could poison another space.
    let pinned = space.ok_or_else(|| Error::EmbeddingFailed {
        message: "this embedding job predates profile pinning; submit a new `memcastle embed` job"
            .into(),
    })?;
    let embeddings = ctx.embeddings().select(profile.as_deref())?;
    if embeddings.space() != Some(&pinned) {
        return Err(Error::EmbeddingFailed {
            message: format!(
                "embedding profile `{}` has changed; restore its original configuration or submit a new embedding job",
                pinned.profile
            ),
        });
    }
    let store = ctx.store();
    // A resumed job continues its running count rather than restarting it.
    let mut done = u64::try_from(JobContext::resume_index(job, "embedded")).unwrap_or(u64::MAX);
    tracing::info!(already_embedded = done, "embedding sweep started");

    let mut after = job
        .checkpoint
        .get("after")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    loop {
        if let Some(stop) = ctx.stop_requested() {
            return Ok(stop);
        }
        let drawers = store
            .embedding_drawer_page(wing.as_deref(), source_id, &after, PASS_SIZE)
            .await?;
        if drawers.is_empty() {
            break;
        }
        let hashes = if refresh == EmbeddingRefresh::All {
            Default::default()
        } else {
            store
                .embedding_hashes_for_page(&drawers, &pinned.fingerprint)
                .await?
        };
        let mut selected = Vec::new();
        for drawer in &drawers {
            let existing = hashes.get(&drawer.id.to_string());
            let needed = match refresh {
                EmbeddingRefresh::Missing => existing.is_none(),
                EmbeddingRefresh::Stale => existing.is_none_or(|(content, input)| {
                    content != &drawer.content_hash
                        || input != &EmbeddingSpace::input_hash(&drawer.content)
                }),
                EmbeddingRefresh::All => true,
            };
            if needed {
                selected.push(drawer);
            }
        }
        let texts: Vec<String> = selected.iter().map(|d| d.content.clone()).collect();
        let vectors = embeddings.embed(&texts).await?;
        let mut stored = 0u64;
        for (drawer, vector) in selected.iter().zip(&vectors) {
            // `false` means the drawer was deleted since it was listed: nothing
            // to embed, and it no longer appears in the next pass either.
            if store
                .set_profile_embedding(
                    drawer,
                    &pinned,
                    &EmbeddingSpace::input_hash(&drawer.content),
                    vector,
                )
                .await?
            {
                stored += 1;
            }
        }
        done += stored;
        after = drawers.last().expect("nonempty page").id.to_string();
        let progress = JobProgress {
            current: u32::try_from(done).unwrap_or(u32::MAX),
            total: None,
            message: Some(format!(
                "embedded {done} drawer(s) into profile `{}`",
                pinned.profile
            )),
        };
        ctx.checkpoint(
            job,
            progress,
            serde_json::json!({ "embedded": done, "after": after }),
        )
        .await?;
        // A pass that stored nothing (every listed drawer vanished) would
        // otherwise spin on an empty page without progress; the next listing
        // is empty anyway, so this only guards a pathological store.
    }
    tracing::info!(embedded = done, "embedding sweep finished");
    Ok(JobOutcome::Completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Drawer, DrawerId, JobKind, Priority, Provenance, RoomId, Source, SourceKind,
    };
    use crate::embed::{EmbedFuture, Embedder, Embeddings, fake};
    use crate::jobs::JobControl;
    use crate::store::SurrealStore;

    async fn seeded(contents: &[&str]) -> (SurrealStore, Vec<Drawer>) {
        let store = SurrealStore::connect_memory_for_tests().await;
        let wing = store.get_or_create_wing("w", None).await.expect("wing");
        let room: RoomId = store
            .get_or_create_room(wing.id, "r", None)
            .await
            .expect("room")
            .id;
        let mut drawers = Vec::new();
        for content in contents {
            let drawer = Drawer::new(
                DrawerId::new(),
                room,
                (*content).to_string(),
                Source {
                    kind: SourceKind::Manual,
                    uri: None,
                    agent: None,
                    origin: None,
                },
                vec![],
                Provenance {
                    requested_by: "test".into(),
                    job_id: None,
                },
            );
            store.create_drawer(&drawer).await.expect("create drawer");
            drawers.push(drawer);
        }
        (store, drawers)
    }

    fn embed_job() -> Job {
        Job::new(
            JobKind::Embed {
                wing: None,
                source_id: None,
                profile: None,
                refresh: EmbeddingRefresh::Missing,
                space: Some(
                    Embeddings::new(fake::WordHashEmbedder, 4)
                        .space()
                        .unwrap()
                        .clone(),
                ),
            },
            Priority::Background,
            "test",
        )
    }

    fn params(wing: Option<String>) -> EmbedParams {
        EmbedParams {
            wing,
            source_id: None,
            profile: None,
            refresh: EmbeddingRefresh::Missing,
            space: Some(
                Embeddings::new(fake::WordHashEmbedder, 4)
                    .space()
                    .unwrap()
                    .clone(),
            ),
        }
    }

    fn context(store: &SurrealStore, job: &Job, embeddings: Embeddings) -> JobContext {
        JobContext::new(job.id, JobControl::default(), store.clone()).with_embeddings(embeddings)
    }

    #[tokio::test]
    async fn the_sweep_embeds_every_drawer_and_leaves_canonical_content_untouched() {
        let contents: Vec<String> = (0..70)
            .map(|i| format!("drawer number {i} about moats"))
            .collect();
        let refs: Vec<&str> = contents.iter().map(String::as_str).collect();
        let (store, drawers) = seeded(&refs).await;
        let mut job = embed_job();
        let ctx = context(&store, &job, Embeddings::new(fake::WordHashEmbedder, 16));

        let outcome = run(&ctx, &mut job, params(None)).await.expect("runs");

        assert_eq!(outcome, JobOutcome::Completed);
        assert_eq!(
            job.progress.current, 70,
            "more than one pass of {PASS_SIZE}"
        );
        let space = params(None).space.unwrap();
        for original in &drawers {
            let stored = store.get_drawer(original.id).await.unwrap().unwrap();
            assert_eq!(stored.content, original.content);
            assert_eq!(stored.content_hash, original.content_hash);
            assert_eq!(stored.updated_at, original.updated_at);
            assert_eq!(
                store
                    .get_profile_embedding(original.id, &space.fingerprint)
                    .await
                    .unwrap()
                    .as_deref(),
                Some(fake::vector_for(&original.content).as_slice())
            );
        }
    }

    #[tokio::test]
    async fn running_the_sweep_again_finds_nothing_to_do() {
        let (store, _) = seeded(&["one", "two"]).await;
        let mut job = embed_job();
        let ctx = context(&store, &job, Embeddings::new(fake::WordHashEmbedder, 4));
        run(&ctx, &mut job, params(None)).await.unwrap();

        let mut again = embed_job();
        let ctx = context(&store, &again, Embeddings::new(fake::WordHashEmbedder, 4));
        let outcome = run(&ctx, &mut again, params(None)).await.unwrap();

        assert_eq!(outcome, JobOutcome::Completed);
        assert_eq!(again.progress.current, 0, "nothing was re-embedded");
    }

    #[tokio::test]
    async fn profile_search_excludes_legacy_vectors_and_unrelated_spaces() {
        let (store, drawers) = seeded(&["moat", "gate"]).await;
        // The old slot has no provenance and is deliberately invisible to profile-aware search.
        store
            .set_drawer_embedding(drawers[1].id, &fake::vector_for("moat"))
            .await
            .unwrap();
        let mut job = embed_job();
        let space = params(None).space.unwrap();
        let ctx = context(&store, &job, Embeddings::new(fake::WordHashEmbedder, 4));
        run(&ctx, &mut job, params(None)).await.unwrap();
        assert_eq!(
            store
                .indexed_embedding_table(&space.fingerprint, 768)
                .await
                .unwrap(),
            Some("embedding_fast_768")
        );
        let hits = store
            .search_vector_space(
                &fake::vector_for("moat"),
                5,
                &crate::domain::SearchFilter::default(),
                &space.fingerprint,
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].drawer.id, drawers[0].id);
        let hybrid = store
            .search_hybrid_space(
                "moat",
                &fake::vector_for("moat"),
                5,
                &crate::domain::SearchFilter::default(),
                &space.fingerprint,
            )
            .await
            .unwrap();
        assert_eq!(hybrid[0].drawer.id, drawers[0].id);
        assert!(hybrid[0].signals.semantic.is_some());
        assert!(
            store
                .search_vector_space(
                    &fake::vector_for("moat"),
                    5,
                    &crate::domain::SearchFilter::default(),
                    "another-space"
                )
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_second_model_of_the_same_dimension_uses_exact_ranking_without_entering_the_first_models_index()
     {
        let (store, drawers) = seeded(&["moat", "gate"]).await;
        let first = params(None).space.unwrap();
        let second = EmbeddingSpace::new(
            "bulk".into(),
            "bulk".into(),
            "openai".into(),
            "connection".into(),
            "different-model".into(),
            768,
        );
        for drawer in &drawers {
            store
                .set_profile_embedding(
                    drawer,
                    &first,
                    &EmbeddingSpace::input_hash(&drawer.content),
                    &fake::vector_for(&drawer.content),
                )
                .await
                .unwrap();
        }
        store
            .set_profile_embedding(
                &drawers[1],
                &second,
                &EmbeddingSpace::input_hash(&drawers[1].content),
                &fake::vector_for("moat"),
            )
            .await
            .unwrap();
        assert!(
            store
                .indexed_embedding_table(&first.fingerprint, 768)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .indexed_embedding_table(&second.fingerprint, 768)
                .await
                .unwrap()
                .is_none()
        );
        let filter = crate::domain::SearchFilter::default();
        let hits = store
            .search_vector_space(&fake::vector_for("moat"), 2, &filter, &second.fingerprint)
            .await
            .unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.drawer.id).collect::<Vec<_>>(),
            vec![drawers[1].id]
        );
        let hybrid = store
            .search_hybrid_space(
                "gate",
                &fake::vector_for("moat"),
                2,
                &filter,
                &second.fingerprint,
            )
            .await
            .unwrap();
        assert!(
            hybrid
                .iter()
                .any(|hit| hit.drawer.id == drawers[1].id && hit.signals.semantic.is_some())
        );
    }

    #[tokio::test]
    async fn page_hash_lookup_ignores_other_spaces_and_reports_only_stored_inputs() {
        let (store, drawers) = seeded(&["one", "two", "three"]).await;
        let selected = params(None).space.unwrap();
        let other = EmbeddingSpace::new(
            "other".into(),
            "other".into(),
            "external".into(),
            "elsewhere".into(),
            "other-model".into(),
            768,
        );
        for (drawer, space) in [(&drawers[0], &selected), (&drawers[1], &other)] {
            store
                .set_profile_embedding(
                    drawer,
                    space,
                    &EmbeddingSpace::input_hash(&drawer.content),
                    &fake::vector_for(&drawer.content),
                )
                .await
                .unwrap();
        }
        let hashes = store
            .embedding_hashes_for_page(&drawers, &selected.fingerprint)
            .await
            .unwrap();
        assert_eq!(hashes.len(), 1);
        assert_eq!(
            hashes[&drawers[0].id.to_string()],
            (
                drawers[0].content_hash.clone(),
                EmbeddingSpace::input_hash(&drawers[0].content)
            )
        );
        assert!(
            store
                .embedding_hashes_for_page(&[], &selected.fingerprint)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn indexed_and_exact_profile_search_apply_wing_scope_before_the_result_limit() {
        let (store, drawers) = seeded(&["gate"]).await;
        let other = store.get_or_create_wing("other", None).await.unwrap();
        let room = store.get_or_create_room(other.id, "r", None).await.unwrap();
        let outsider = Drawer::new(
            DrawerId::new(),
            room.id,
            "moat".into(),
            Source::new(SourceKind::Manual, None, None),
            vec![],
            Provenance {
                requested_by: "test".into(),
                job_id: None,
            },
        );
        store.create_drawer(&outsider).await.unwrap();
        let indexed = params(None).space.unwrap();
        let exact = EmbeddingSpace::new(
            "other".into(),
            "other".into(),
            "external".into(),
            "elsewhere".into(),
            "other-model".into(),
            768,
        );
        for space in [&indexed, &exact] {
            for drawer in [&drawers[0], &outsider] {
                store
                    .set_profile_embedding(
                        drawer,
                        space,
                        &EmbeddingSpace::input_hash(&drawer.content),
                        &fake::vector_for(&drawer.content),
                    )
                    .await
                    .unwrap();
            }
        }
        let filter = crate::domain::SearchFilter {
            wing: Some("w".into()),
            ..Default::default()
        };
        for space in [&indexed, &exact] {
            let hits = store
                .search_vector_space(&fake::vector_for("moat"), 1, &filter, &space.fingerprint)
                .await
                .unwrap();
            assert_eq!(
                hits.iter().map(|hit| hit.drawer.id).collect::<Vec<_>>(),
                [drawers[0].id]
            );
        }
    }

    #[tokio::test]
    async fn an_arbitrary_dimension_has_a_profile_filtered_exact_path() {
        let (store, drawers) = seeded(&["moat"]).await;
        let space = EmbeddingSpace::new(
            "tiny".into(),
            "tiny".into(),
            "external".into(),
            "client".into(),
            "small-model".into(),
            3,
        );
        store
            .set_profile_embedding(
                &drawers[0],
                &space,
                &EmbeddingSpace::input_hash(&drawers[0].content),
                &[1.0, 0.0, 0.0],
            )
            .await
            .unwrap();
        assert!(
            store
                .indexed_embedding_table(&space.fingerprint, 3)
                .await
                .unwrap()
                .is_none()
        );
        let hits = store
            .search_vector_space(
                &[1.0, 0.0, 0.0],
                1,
                &crate::domain::SearchFilter::default(),
                &space.fingerprint,
            )
            .await
            .unwrap();
        assert_eq!(hits[0].drawer.id, drawers[0].id);
    }

    #[tokio::test]
    async fn an_unpinned_legacy_job_or_a_changed_model_never_embeds_into_a_different_space() {
        let (store, drawers) = seeded(&["one"]).await;
        let embeddings = Embeddings::new(fake::WordHashEmbedder, 4);
        let mut job = embed_job();
        let ctx = context(&store, &job, embeddings.clone());
        let mut old = params(None);
        old.space = None;
        let error = run(&ctx, &mut job, old).await.unwrap_err();
        assert!(error.to_string().contains("predates profile pinning"));
        let mut changed = params(None);
        changed.space.as_mut().unwrap().model = "changed-model".into();
        let error = run(&ctx, &mut job, changed).await.unwrap_err();
        assert!(error.to_string().contains("has changed"));
        assert!(
            store
                .embedding_hashes(drawers[0].id, &embeddings.space().unwrap().fingerprint)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn stale_and_all_refreshes_are_explicit_and_missing_only_does_not_replace_a_vector() {
        let (store, drawers) = seeded(&["one"]).await;
        let mut job = embed_job();
        let ctx = context(&store, &job, Embeddings::new(fake::WordHashEmbedder, 4));
        run(&ctx, &mut job, params(None)).await.unwrap();
        let space = params(None).space.unwrap();
        let original = store
            .get_profile_embedding(drawers[0].id, &space.fingerprint)
            .await
            .unwrap()
            .unwrap();
        // A caller may have supplied an old vector in this same space; missing-only cannot replace it.
        store
            .set_profile_embedding(
                &drawers[0],
                &space,
                "stale-input-hash",
                &fake::vector_for("another"),
            )
            .await
            .unwrap();
        let mut missing = embed_job();
        run(
            &context(&store, &missing, Embeddings::new(fake::WordHashEmbedder, 4)),
            &mut missing,
            params(None),
        )
        .await
        .unwrap();
        assert_eq!(missing.progress.current, 0);
        let mut stale = embed_job();
        let mut refresh = params(None);
        refresh.refresh = EmbeddingRefresh::Stale;
        run(
            &context(&store, &stale, Embeddings::new(fake::WordHashEmbedder, 4)),
            &mut stale,
            refresh,
        )
        .await
        .unwrap();
        assert_eq!(stale.progress.current, 1);
        assert_eq!(
            store
                .get_profile_embedding(drawers[0].id, &space.fingerprint)
                .await
                .unwrap()
                .unwrap(),
            original
        );
        let mut all = embed_job();
        let mut refresh = params(None);
        refresh.refresh = EmbeddingRefresh::All;
        run(
            &context(&store, &all, Embeddings::new(fake::WordHashEmbedder, 4)),
            &mut all,
            refresh,
        )
        .await
        .unwrap();
        assert_eq!(all.progress.current, 1);
    }

    #[tokio::test]
    async fn a_vector_derived_from_outdated_content_cannot_be_written_after_the_drawer_changes() {
        let (store, drawers) = seeded(&["original"]).await;
        let mut stale = drawers[0].clone();
        stale.content_hash = "outdated".into();
        let space = params(None).space.unwrap();
        assert!(
            !store
                .set_profile_embedding(
                    &stale,
                    &space,
                    &EmbeddingSpace::input_hash(&stale.content),
                    &fake::vector_for(&stale.content)
                )
                .await
                .unwrap()
        );
        assert!(
            store
                .get_profile_embedding(stale.id, &space.fingerprint)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn supersession_never_uses_the_old_vectors_for_the_new_current_content() {
        let (store, drawers) = seeded(&["original"]).await;
        let old = &drawers[0];
        let space = params(None).space.unwrap();
        store
            .set_profile_embedding(
                old,
                &space,
                &EmbeddingSpace::input_hash(&old.content),
                &fake::vector_for(&old.content),
            )
            .await
            .unwrap();
        let replacement = Drawer::new(
            DrawerId::new(),
            old.room,
            "replacement".into(),
            Source::new(SourceKind::Manual, None, None),
            vec![],
            Provenance {
                requested_by: "test".into(),
                job_id: None,
            },
        );
        store
            .supersede_drawer(old.id, Some(&replacement), chrono::Utc::now())
            .await
            .unwrap();
        let query = fake::vector_for(&old.content);
        let current = crate::domain::SearchFilter::default();
        assert!(
            store
                .search_vector_space(&query, 10, &current, &space.fingerprint)
                .await
                .unwrap()
                .is_empty()
        );
        let hybrid = store
            .search_hybrid_space("replacement", &query, 10, &current, &space.fingerprint)
            .await
            .unwrap();
        assert_eq!(hybrid[0].drawer.id, replacement.id);
        assert!(
            hybrid[0].signals.semantic.is_none(),
            "the old vector must not contribute to a current replacement"
        );
        let historical = crate::domain::SearchFilter {
            temporal: crate::domain::Temporal::All,
            ..Default::default()
        };
        let old_hits = store
            .search_vector_space(&query, 10, &historical, &space.fingerprint)
            .await
            .unwrap();
        assert_eq!(
            old_hits[0].drawer.id, old.id,
            "historical vector stays tied to its immutable content"
        );
        store
            .set_profile_embedding(
                &replacement,
                &space,
                &EmbeddingSpace::input_hash(&replacement.content),
                &fake::vector_for(&replacement.content),
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .search_vector_space(
                    &fake::vector_for(&replacement.content),
                    10,
                    &current,
                    &space.fingerprint
                )
                .await
                .unwrap()[0]
                .drawer
                .id,
            replacement.id
        );
    }

    #[tokio::test]
    async fn a_miner_sweep_without_a_wing_only_embeds_drawers_from_its_pinned_source() {
        let (store, _) = seeded(&["unrelated manual note"]).await;
        let wing = store.get_wing("w").await.unwrap().unwrap();
        let room = store.get_room(wing.id, "r").await.unwrap().unwrap();
        let source_id = crate::domain::SourceId::new();
        let mined = Drawer::new(
            DrawerId::new(),
            room.id,
            "mined document".into(),
            Source {
                kind: SourceKind::File,
                uri: None,
                agent: None,
                origin: Some(crate::domain::Origin {
                    source_id,
                    source: "directory".into(),
                    document: "doc".into(),
                    chunk: 0,
                    revision: "one".into(),
                    metadata: None,
                    occurred_at: None,
                }),
            },
            vec![],
            Provenance {
                requested_by: "test".into(),
                job_id: None,
            },
        );
        store.create_drawer(&mined).await.unwrap();
        let mut job = embed_job();
        let mut selected = params(None);
        selected.source_id = Some(source_id);
        run(
            &context(&store, &job, Embeddings::new(fake::WordHashEmbedder, 4)),
            &mut job,
            selected,
        )
        .await
        .unwrap();
        let space = params(None).space.unwrap();
        assert!(
            store
                .get_profile_embedding(mined.id, &space.fingerprint)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(job.progress.current, 1);
    }

    #[tokio::test]
    async fn an_interrupted_refresh_resumes_from_its_durable_checkpoint_without_repeating_a_page() {
        struct PauseAfterFirstBatch(JobControl);
        impl Embedder for PauseAfterFirstBatch {
            fn embed<'a>(&'a self, texts: &'a [String]) -> EmbedFuture<'a> {
                let vectors = texts.iter().map(|text| fake::vector_for(text)).collect();
                self.0.request_pause();
                Box::pin(async move { Ok(vectors) })
            }
        }
        let contents: Vec<String> = (0..40).map(|n| format!("item {n}")).collect();
        let (store, drawers) =
            seeded(&contents.iter().map(String::as_str).collect::<Vec<_>>()).await;
        let mut job = embed_job();
        job.apply(crate::domain::JobEvent::Claim).unwrap();
        store.save_job(&job).await.unwrap();
        let control = JobControl::default();
        let ctx = JobContext::new(job.id, control.clone(), store.clone())
            .with_embeddings(Embeddings::new(PauseAfterFirstBatch(control), 32));
        assert_eq!(
            run(&ctx, &mut job, params(None)).await.unwrap(),
            JobOutcome::Paused
        );
        assert_eq!(job.progress.current, 32);
        let mut resumed = store.get_job(job.id).await.unwrap().unwrap();
        assert_eq!(resumed.checkpoint["after"], job.checkpoint["after"]);
        let ctx = context(
            &store,
            &resumed,
            Embeddings::new(fake::WordHashEmbedder, 32),
        );
        assert_eq!(
            run(&ctx, &mut resumed, params(None)).await.unwrap(),
            JobOutcome::Completed
        );
        assert_eq!(resumed.progress.current, 40);
        let space = params(None).space.unwrap();
        for drawer in drawers {
            assert!(
                store
                    .get_profile_embedding(drawer.id, &space.fingerprint)
                    .await
                    .unwrap()
                    .is_some()
            );
        }
    }

    #[tokio::test]
    async fn a_sweep_without_a_provider_fails_with_an_actionable_error() {
        let (store, _) = seeded(&["one"]).await;
        let mut job = embed_job();
        let ctx = context(&store, &job, Embeddings::disabled());

        let error = run(&ctx, &mut job, params(None))
            .await
            .expect_err("no provider");

        assert!(matches!(error, Error::EmbeddingsNotConfigured), "{error}");
    }

    #[tokio::test]
    async fn a_pause_request_stops_the_sweep_before_any_work() {
        let (store, _) = seeded(&["one", "two"]).await;
        let mut job = embed_job();
        let control = JobControl::default();
        control.request_pause();
        let ctx = JobContext::new(job.id, control, store.clone())
            .with_embeddings(Embeddings::new(fake::WordHashEmbedder, 4));

        let outcome = run(&ctx, &mut job, params(None)).await.unwrap();

        assert_eq!(outcome, JobOutcome::Paused);
        assert_eq!(
            store
                .list_drawers_without_embedding(None, 10)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn a_failing_provider_fails_the_job_and_changes_no_drawer() {
        struct Broken;
        impl Embedder for Broken {
            fn embed<'a>(&'a self, _texts: &'a [String]) -> EmbedFuture<'a> {
                Box::pin(async {
                    Err(Error::EmbeddingFailed {
                        message: "quota".into(),
                    })
                })
            }
        }
        let (store, _) = seeded(&["one"]).await;
        let mut job = embed_job();
        let ctx = context(&store, &job, Embeddings::new(Broken, 4));

        let error = run(&ctx, &mut job, params(None))
            .await
            .expect_err("provider fails");

        assert!(error.to_string().contains("quota"), "{error}");
        assert_eq!(
            store
                .list_drawers_without_embedding(None, 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_wing_scoped_sweep_leaves_other_wings_alone() {
        let (store, _) = seeded(&["in w"]).await;
        let other = store.get_or_create_wing("other", None).await.unwrap();
        let room = store.get_or_create_room(other.id, "r", None).await.unwrap();
        let outsider = Drawer::new(
            DrawerId::new(),
            room.id,
            "elsewhere".to_string(),
            Source {
                kind: SourceKind::Manual,
                uri: None,
                agent: None,
                origin: None,
            },
            vec![],
            Provenance {
                requested_by: "test".into(),
                job_id: None,
            },
        );
        store.create_drawer(&outsider).await.unwrap();
        let mut job = embed_job();
        let ctx = context(&store, &job, Embeddings::new(fake::WordHashEmbedder, 4));

        run(&ctx, &mut job, params(Some("w".into()))).await.unwrap();

        let space = params(None).space.unwrap();
        assert!(
            store
                .get_profile_embedding(outsider.id, &space.fingerprint)
                .await
                .unwrap()
                .is_none()
        );
    }
}
