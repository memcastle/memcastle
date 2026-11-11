//! Provider-neutral lifecycle regressions: no remote service or credential needed.
use super::*;
use crate::domain::{
    Candidate, Cursor, JobEvent, JobKind, KnownDocument, MiningPreset, MiningSource, Options,
    Priority, Segment, SourceCapabilities, SourceKind, SourceRef,
};
use crate::jobs::JobControl;
use std::collections::BTreeMap;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct Reconciler {
    documents: Mutex<BTreeMap<String, String>>,
    active: AtomicUsize,
    peak: AtomicUsize,
}

impl Reconciler {
    fn new() -> Self {
        Self {
            documents: Mutex::new(BTreeMap::from([
                ("a".into(), "Alpha knowledge".into()),
                ("b".into(), "Beta knowledge".into()),
            ])),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }
}

impl SourceAdapter for Reconciler {
    fn name(&self) -> &str {
        "reconciler"
    }
    fn description(&self) -> &str {
        "Synthetic reconciliation source"
    }
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            incremental: true,
            ..Default::default()
        }
    }
    fn reconciles(&self) -> bool {
        true
    }
    fn identify(&self, _: Option<&str>, options: &Options) -> Result<SourceRef> {
        Ok(SourceRef::new(self.name(), None, "shared-workspace").with_options(options.clone()))
    }
    fn default_wing(&self, _: &SourceRef) -> String {
        "knowledge".into()
    }
    fn default_room(&self) -> &str {
        "pages"
    }
    async fn discover(
        &self,
        source: &SourceRef,
        cursor: &Cursor,
        limit: usize,
    ) -> Result<Discovery> {
        if cursor.is_null() {
            return Ok(Discovery {
                cursor_after: Some(json!({"after":""})),
                ..Default::default()
            });
        }
        let after = cursor["after"]
            .as_str()
            .ok_or_else(|| crate::Error::SourceCursorInvalid {
                adapter: self.name().into(),
                message: "invalid test cursor".into(),
            })?;
        let documents = self.documents.lock().unwrap();
        let remaining = documents
            .keys()
            .filter(|id| id.as_str() > after && source.options.get("exclude") != Some(id))
            .collect::<Vec<_>>();
        let exhausted = remaining.len() <= limit;
        let candidates = remaining
            .into_iter()
            .take(limit)
            .map(|id| Candidate {
                external_id: id.clone(),
                handle: id.clone(),
                cursor_after: json!({"after":id}),
            })
            .collect();
        Ok(Discovery {
            candidates,
            exhausted,
            ..Default::default()
        })
    }
    async fn reconcile(&self, source: &SourceRef, known: &KnownDocument) -> Result<Reconciliation> {
        if source.options.get("exclude") == Some(&known.external_id)
            || !self
                .documents
                .lock()
                .unwrap()
                .contains_key(&known.external_id)
        {
            return Ok(Reconciliation::Retire("outside_scope_or_deleted".into()));
        }
        Ok(Reconciliation::Refresh(known.external_id.clone()))
    }
    async fn acquire(&self, source: &SourceRef, candidate: &Candidate) -> Result<Acquisition> {
        if source.options.get("fail") == Some(&candidate.external_id) {
            return Err(crate::Error::SourceFailed {
                name: self.name().into(),
                message: "transient test failure".into(),
            });
        }
        if source.options.get("skip") == Some(&candidate.external_id) {
            return Ok(Acquisition::Skip);
        }
        Ok(self
            .read(source, candidate)
            .await?
            .map_or(Acquisition::Retire("deleted".into()), Acquisition::Document))
    }
    async fn read(&self, _: &SourceRef, candidate: &Candidate) -> Result<Option<RawDocument>> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(self
            .documents
            .lock()
            .unwrap()
            .get(&candidate.external_id)
            .map(|body| RawDocument {
                external_id: candidate.external_id.clone(),
                revision: RawDocument::revision_of(body),
                body: body.clone(),
                metadata: json!({}),
                occurred_at: None,
            }))
    }
    fn normalize(&self, raw: &RawDocument) -> Result<CanonicalDocument> {
        Ok(CanonicalDocument {
            title: Some(raw.external_id.clone()),
            room: None,
            name: None,
            kind: SourceKind::Other,
            uri: None,
            tags: vec![],
            segments: vec![Segment {
                text: raw.body.clone(),
            }],
        })
    }
}

async fn execute(
    adapter: &Reconciler,
    store: &SurrealStore,
    options: Options,
    miner: Option<&str>,
    full: bool,
    cap: Option<usize>,
) -> (Job, Result<JobOutcome>) {
    let preset = MiningPreset {
        miner: miner.map(str::to_string),
        max_documents: cap,
        ..Default::default()
    };
    let mut job = Job::new(
        JobKind::Mine {
            source: MiningSource::Named {
                source: adapter.name().into(),
                locator: None,
            },
            wing: None,
            full,
            options: options.clone(),
            embedding_space: None,
            preset: Some(Box::new(preset.clone())),
        },
        Priority::Background,
        "reconciliation-test",
    );
    job.apply(JobEvent::Claim).unwrap();
    let ctx = JobContext::new(job.id, JobControl::default(), store.clone());
    let outcome = mine(
        adapter,
        &ctx,
        &mut job,
        Request {
            locator: None,
            options: &options,
            wing: None,
            full,
            preset: Some(&preset),
        },
    )
    .await;
    (job, outcome)
}

fn source_id(job: &Job) -> crate::domain::SourceId {
    serde_json::from_value(job.result.as_ref().unwrap()["source_id"].clone()).unwrap()
}

#[tokio::test]
async fn full_runs_reconcile_old_ids_retire_explicitly_and_restore_the_same_revision() {
    let store = SurrealStore::connect_memory_for_tests().await;
    let adapter = Reconciler::new();
    let (first, result) = execute(&adapter, &store, Options::new(), None, false, None).await;
    result.unwrap();
    let source = source_id(&first);
    let old = store
        .get_source_document(source, "a")
        .await
        .unwrap()
        .unwrap();
    let (retired, result) = execute(
        &adapter,
        &store,
        Options::from([("exclude".into(), "a".into())]),
        None,
        true,
        None,
    )
    .await;
    result.unwrap();
    assert_eq!(source_id(&retired), source);
    let record = store
        .get_source_document(source, "a")
        .await
        .unwrap()
        .unwrap();
    assert!(record.retired_at.is_some());
    assert!(record.chunks.is_empty());
    assert!(
        store
            .get_drawer(old.chunks[0].drawer)
            .await
            .unwrap()
            .unwrap()
            .valid_to
            .is_some()
    );
    let (_, result) = execute(&adapter, &store, Options::new(), None, true, None).await;
    result.unwrap();
    let restored = store
        .get_source_document(source, "a")
        .await
        .unwrap()
        .unwrap();
    assert!(restored.retired_at.is_none());
    assert_eq!(restored.revision, old.revision);
    assert_ne!(restored.chunks[0].drawer, old.chunks[0].drawer);
    let (unchanged, result) = execute(&adapter, &store, Options::new(), None, true, None).await;
    result.unwrap();
    assert_eq!(unchanged.result.unwrap()["created"], 0);
}

#[tokio::test]
async fn a_skip_keeps_existing_content_and_a_failure_never_advances_past_acquisition() {
    let store = SurrealStore::connect_memory_for_tests().await;
    let adapter = Reconciler::new();
    let (first, result) = execute(&adapter, &store, Options::new(), None, false, None).await;
    result.unwrap();
    let source = source_id(&first);
    let old = store
        .get_source_document(source, "a")
        .await
        .unwrap()
        .unwrap();
    let (_, result) = execute(
        &adapter,
        &store,
        Options::from([("skip".into(), "a".into())]),
        None,
        false,
        None,
    )
    .await;
    result.unwrap();
    assert!(
        store
            .get_drawer(old.chunks[0].drawer)
            .await
            .unwrap()
            .unwrap()
            .valid_to
            .is_none()
    );
    let (_, result) = execute(
        &adapter,
        &store,
        Options::from([("fail".into(), "a".into())]),
        None,
        false,
        None,
    )
    .await;
    assert!(result.is_err());
    let scan = store
        .source_reconciliation_state(source)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(scan["after"], serde_json::Value::Null);
    assert!(
        store
            .get_source_document(source, "a")
            .await
            .unwrap()
            .unwrap()
            .retired_at
            .is_none()
    );
}

#[tokio::test]
async fn registration_covers_a_crash_before_the_first_document_record() {
    let store = SurrealStore::connect_memory_for_tests().await;
    let adapter = Reconciler::new();
    let (_, result) = execute(
        &adapter,
        &store,
        Options::from([("fail".into(), "a".into())]),
        None,
        false,
        None,
    )
    .await;
    assert!(result.is_err());
    let source = adapter.identify(None, &Options::new()).unwrap().id();
    let known = store
        .source_inventory_page(source, None, 256)
        .await
        .unwrap();
    assert!(
        known
            .iter()
            .any(|known| known.external_id == "a" && known.pending)
    );
    assert_eq!(
        store.get_source(source).await.unwrap().unwrap().cursor,
        json!({"after":""})
    );
    // A drawer can precede the document record. Retirement must find it by origin,
    // not rely on a chunk list that the interrupted attempt never saved.
    let drawer = Drawer::new(
        DrawerId::new(),
        crate::domain::RoomId::new(),
        "orphaned acquisition".into(),
        Source {
            kind: crate::domain::SourceKind::Other,
            uri: None,
            agent: None,
            origin: Some(Origin {
                source_id: source,
                source: adapter.name().into(),
                document: "a".into(),
                chunk: 0,
                revision: "interrupted".into(),
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
    store.create_drawer(&drawer).await.unwrap();
    adapter.documents.lock().unwrap().remove("a");
    let (_, result) = execute(&adapter, &store, Options::new(), None, true, None).await;
    result.unwrap();
    assert!(
        store
            .get_drawer(drawer.id)
            .await
            .unwrap()
            .unwrap()
            .valid_to
            .is_some()
    );
    let retired = store
        .source_inventory_page(source, None, 256)
        .await
        .unwrap();
    assert!(
        retired
            .iter()
            .any(|known| known.external_id == "a" && known.pending && known.retired)
    );
    adapter
        .documents
        .lock()
        .unwrap()
        .insert("a".into(), "Restored acquisition".into());
    let (_, result) = execute(&adapter, &store, Options::new(), None, true, None).await;
    result.unwrap();
    let restored = store
        .source_inventory_page(source, None, 256)
        .await
        .unwrap();
    assert!(
        restored
            .iter()
            .any(|known| known.external_id == "a" && !known.pending && !known.retired)
    );
}

#[tokio::test]
async fn small_document_limits_resume_the_core_inventory_instead_of_restarting_it() {
    let store = SurrealStore::connect_memory_for_tests().await;
    let adapter = Reconciler::new();
    for _cycle in 0..2 {
        let mut complete = false;
        for _ in 0..10 {
            let (job, result) =
                execute(&adapter, &store, Options::new(), None, false, Some(1)).await;
            result.unwrap();
            if job.result.unwrap()["truncated"] == false {
                complete = true;
                break;
            }
        }
        assert!(
            complete,
            "inventory and discovery must both reach completion under a one-document cap"
        );
    }
    assert_eq!(
        store
            .list_drawers(None)
            .await
            .unwrap()
            .iter()
            .filter(|drawer| drawer.valid_to.is_none())
            .count(),
        2
    );
}

#[tokio::test]
async fn miners_with_the_same_locator_account_and_destination_have_independent_histories() {
    let store = SurrealStore::connect_memory_for_tests().await;
    let adapter = Reconciler::new();
    let (first, result) = execute(&adapter, &store, Options::new(), Some("one"), false, None).await;
    result.unwrap();
    let (second, result) =
        execute(&adapter, &store, Options::new(), Some("two"), false, None).await;
    result.unwrap();
    assert_ne!(source_id(&first), source_id(&second));
    let (narrowed, result) = execute(
        &adapter,
        &store,
        Options::from([("exclude".into(), "a".into())]),
        Some("one"),
        false,
        None,
    )
    .await;
    result.unwrap();
    assert_eq!(source_id(&first), source_id(&narrowed));
    assert!(
        store
            .get_source_document(source_id(&second), "a")
            .await
            .unwrap()
            .unwrap()
            .retired_at
            .is_none()
    );
}

#[tokio::test]
async fn concurrent_jobs_for_one_source_do_not_overlap_acquisition_or_lose_progress() {
    let store = SurrealStore::connect_memory_for_tests().await;
    let adapter = Reconciler::new();
    let (first, second) = tokio::join!(
        execute(&adapter, &store, Options::new(), None, false, None),
        execute(&adapter, &store, Options::new(), None, false, None)
    );
    first.1.unwrap();
    second.1.unwrap();
    assert_eq!(source_id(&first.0), source_id(&second.0));
    assert!(
        adapter.peak.load(Ordering::SeqCst) <= 2,
        "the two-read window must not multiply across jobs for the same source"
    );
    assert_eq!(store.list_drawers(None).await.unwrap().len(), 2);
}
