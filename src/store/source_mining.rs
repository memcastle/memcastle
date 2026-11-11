//! Cross-daemon mining admission and transactional fencing of canonical writes.
use super::SurrealStore;
use crate::domain::{Job, JobId, SourceId};
use crate::error::{Error, Result};

#[derive(Clone)]
pub(super) struct Fence {
    source: SourceId,
    pub(super) job: JobId,
    worker: Option<String>,
    attempt: u32,
    token: String,
}

impl SurrealStore {
    /// A source has one writer across daemons, not merely one task per process.
    /// A dead owner's lock is reclaimable only after its durable job lease ends.
    pub(crate) async fn claim_mining_source(
        &self,
        source: SourceId,
        job: &Job,
        worker: Option<&str>,
    ) -> Result<Option<Self>> {
        let fence = Fence {
            source,
            job: job.id,
            worker: worker.map(str::to_string),
            attempt: job.attempt,
            token: uuid::Uuid::new_v4().to_string(),
        };
        let claimed = super::retrying_on_conflict(|| async {
            let mut response = self.db.query(
                "BEGIN TRANSACTION; \
                 LET $lock = (SELECT * FROM type::record('source_mining_lock', $source))[0]; \
                 LET $owner = (SELECT * FROM type::record('job', $lock.job ?? $job))[0]; \
                 IF $lock != NONE AND ($lock.worker = NONE OR \
                     ($owner.status = 'running' AND $owner.lease_owner = $lock.worker AND $owner.attempt = $lock.attempt \
                         AND <datetime>($owner.lease_expires_at ?? '1970-01-01T00:00:00Z') > time::now())) \
                     { THROW 'mining source busy'; }; \
                 UPSERT type::record('source_mining_lock', $source) SET job = $job, worker = $worker, \
                     attempt = $attempt, token = $mining_token, serial = 0; \
                 COMMIT TRANSACTION;"
            ).bind(("source", source.to_string())).bind(("job", fence.job.to_string()))
                .bind(("worker", fence.worker.clone())).bind(("attempt", fence.attempt)).bind(("mining_token", fence.token.clone())).await?;
            let errors = response.take_errors();
            if errors.values().any(|error| error.to_string().contains("mining source busy")) { return Ok(false); }
            if let Some(error) = super::root_cause(errors) { return Err(error.into()); }
            Ok(true)
        }).await?;
        Ok(claimed.then(|| Self {
            db: self.db.clone(),
            mining_fence: Some(fence),
        }))
    }

    /// A stale owner cannot release a successor's lock. Also used after failures
    /// and cooperative stops; force-aborted tasks are recovered through the job lease.
    pub(crate) async fn release_mining_source(&self) -> Result<()> {
        if let Some(fence) = &self.mining_fence {
            self.db.query("DELETE source_mining_lock WHERE id = type::record('source_mining_lock', $source) AND token = $mining_token")
                .bind(("source", fence.source.to_string())).bind(("mining_token", fence.token.clone())).await?.check()?;
        }
        Ok(())
    }

    /// Guard and mutation are one transaction. Updating the lock's serial makes
    /// ownership transfer conflict with an old transaction, rather than only reading
    /// a potentially stale snapshot of the lock before an unfenced write.
    pub(super) fn mining_query(
        &self,
        sql: impl Into<String>,
    ) -> surrealdb::method::Query<'_, surrealdb::engine::any::Any> {
        let sql = sql.into();
        let Some(fence) = &self.mining_fence else {
            return self.db.query(sql);
        };
        let body = sql
            .trim()
            .strip_prefix("BEGIN TRANSACTION;")
            .unwrap_or(sql.trim())
            .trim();
        let body = body.strip_suffix("COMMIT TRANSACTION;").unwrap_or(body);
        self.db.query(format!(
            "BEGIN TRANSACTION; \
             LET $__locked = (UPDATE source_mining_lock SET serial += 1 \
                 WHERE id = type::record('source_mining_lock', $__source) AND token = $__token \
                     AND job = $__job AND attempt = $__attempt RETURN AFTER); \
             IF array::len($__locked) != 1 {{ THROW 'mining lease lost'; }}; \
             LET $__owner = (SELECT * FROM type::record('job', $__job))[0]; \
             IF $__worker != NONE AND ($__owner.status != 'running' OR $__owner.lease_owner != $__worker \
                 OR $__owner.attempt != $__attempt OR $__owner.lease_expires_at = NONE \
                 OR <datetime>($__owner.lease_expires_at ?? '1970-01-01T00:00:00Z') <= time::now()) {{ THROW 'mining lease lost'; }}; \
             {body}; COMMIT TRANSACTION;"
        )).bind(("__source", fence.source.to_string())).bind(("__token", fence.token.clone()))
            .bind(("__job", fence.job.to_string())).bind(("__worker", fence.worker.clone())).bind(("__attempt", fence.attempt))
    }

    pub(super) fn checked_mining(
        &self,
        mut response: surrealdb::IndexedResults,
    ) -> Result<surrealdb::IndexedResults> {
        let errors = response.take_errors();
        if errors
            .values()
            .any(|error| error.to_string().contains("mining lease lost"))
        {
            return Err(Error::LeaseLost {
                id: self
                    .mining_fence
                    .as_ref()
                    .expect("only a fenced query raises this error")
                    .job
                    .to_string(),
            });
        }
        match super::root_cause(errors) {
            Some(error) => Err(error.into()),
            None => Ok(response),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Drawer, DrawerId, JobEvent, JobKind, Priority, Provenance, RoomId, Source, SourceKind,
        SourceRef,
    };
    use chrono::Utc;
    use serde_json::json;

    async fn job(store: &SurrealStore, worker: &str) -> Job {
        let mut job = Job::new(
            JobKind::Demo { steps: 1 },
            Priority::Normal,
            "source-fence-test",
        );
        job.apply(JobEvent::Claim).unwrap();
        job.attempt += 1;
        job.lease_owner = Some(worker.into());
        job.lease_expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
        store.save_job(&job).await.unwrap();
        job
    }

    #[tokio::test]
    async fn source_admission_is_shared_and_expired_workers_cannot_write_or_release_a_successor() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let source = store
            .get_or_create_source(&SourceRef::new("fixture", None, "workspace"), None)
            .await
            .unwrap();
        let a = job(&store, "worker-a").await;
        let b = job(&store, "worker-b").await;
        let old = store
            .claim_mining_source(source.id, &a, Some("worker-a"))
            .await
            .unwrap()
            .unwrap();
        assert!(
            store
                .claim_mining_source(source.id, &b, Some("worker-b"))
                .await
                .unwrap()
                .is_none()
        );
        old.save_source_cursor(source.id, &json!({"page":1}), a.id, Utc::now())
            .await
            .unwrap();
        // Reclaim through the same durable lease predicate remote daemons use.
        store
            .renew_job_lease(a.id, "worker-a", Utc::now() - chrono::Duration::seconds(1))
            .await
            .unwrap();
        let current = store
            .claim_mining_source(source.id, &b, Some("worker-b"))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            old.save_source_cursor(source.id, &json!({"page":999}), a.id, Utc::now())
                .await,
            Err(Error::LeaseLost { .. })
        ));
        assert!(matches!(
            old.register_source_document(source.id, "stale").await,
            Err(Error::LeaseLost { .. })
        ));
        let drawer = Drawer::new(
            DrawerId::new(),
            RoomId::new(),
            "stale body".into(),
            Source::new(SourceKind::Other, None, None),
            vec![],
            Provenance {
                requested_by: "test".into(),
                job_id: Some(a.id),
            },
        );
        assert!(matches!(
            old.create_drawer(&drawer).await,
            Err(Error::LeaseLost { .. })
        ));
        assert!(store.list_drawers(None).await.unwrap().is_empty());
        assert_eq!(
            store.get_source(source.id).await.unwrap().unwrap().cursor,
            json!({"page":1})
        );
        old.release_mining_source().await.unwrap();
        assert!(
            store
                .claim_mining_source(source.id, &a, Some("worker-a"))
                .await
                .unwrap()
                .is_none(),
            "an old release must not delete the new token"
        );
        current
            .save_source_cursor(source.id, &json!({"page":2}), b.id, Utc::now())
            .await
            .unwrap();
        current.release_mining_source().await.unwrap();
    }

    #[tokio::test]
    async fn different_effective_sources_do_not_share_an_admission_lock() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let a = job(&store, "worker-a").await;
        let b = job(&store, "worker-b").await;
        let first = store
            .claim_mining_source(SourceId::new(), &a, Some("worker-a"))
            .await
            .unwrap()
            .unwrap();
        let second = store
            .claim_mining_source(SourceId::new(), &b, Some("worker-b"))
            .await
            .unwrap()
            .unwrap();
        first.release_mining_source().await.unwrap();
        second.release_mining_source().await.unwrap();
    }

    #[tokio::test]
    async fn a_recovered_attempt_can_reclaim_its_own_stale_source_lock() {
        let store = SurrealStore::connect_memory_for_tests().await;
        let source = store
            .get_or_create_source(&SourceRef::new("fixture", None, "recovered"), None)
            .await
            .unwrap();
        let mut recovered = job(&store, "worker-a").await;
        let old = store
            .claim_mining_source(source.id, &recovered, Some("worker-a"))
            .await
            .unwrap()
            .unwrap();
        recovered.apply(JobEvent::RecoverToQueued).unwrap();
        store.save_job(&recovered).await.unwrap();
        recovered.apply(JobEvent::Claim).unwrap();
        recovered.attempt += 1;
        recovered.lease_owner = Some("worker-a".into());
        recovered.lease_expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
        store.save_job(&recovered).await.unwrap();
        let current = store
            .claim_mining_source(source.id, &recovered, Some("worker-a"))
            .await
            .unwrap()
            .expect("the old attempt must not mistake the recovered job's new lease for its own");
        assert!(matches!(
            old.register_source_document(source.id, "stale-attempt")
                .await,
            Err(Error::LeaseLost { .. })
        ));
        old.release_mining_source().await.unwrap();
        current
            .register_source_document(source.id, "recovered-attempt")
            .await
            .unwrap();
        current.release_mining_source().await.unwrap();
    }
}
