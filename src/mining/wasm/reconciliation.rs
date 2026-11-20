//! Optional typed exports share source@0.5.0's async acquisition contract.
use super::{Inner, to_wit, wit};
use crate::domain::{Candidate, Cursor, SourceRef};
use crate::error::Result;
use crate::mining::adapter::{Acquisition, Discovery, KnownDocument, Reconciliation};
use wasmtime::component::{Component, ComponentExportIndex, ComponentType, Lift, Lower};

pub(super) struct Extension {
    discover: ComponentExportIndex,
    reconcile: ComponentExportIndex,
    acquire: ComponentExportIndex,
}

impl Extension {
    pub fn load(
        component: &Component,
        version: Option<&str>,
    ) -> std::result::Result<Option<Self>, String> {
        let Some(version) = version else {
            return Ok(None);
        };
        if version != "0.1.0" {
            return Err("unsupported source reconciliation extension; use compatibility.reconciliation = '0.1.0'".into());
        }
        let interface = component
            .get_export_index(None, "memcastle:source/reconciliation@0.5.0")
            .ok_or("source declares reconciliation but does not export its interface")?;
        let export = |name| {
            component
                .get_export_index(Some(&interface), name)
                .ok_or_else(|| format!("source reconciliation interface has no `{name}` export"))
        };
        Ok(Some(Self {
            discover: export("discover")?,
            reconcile: export("reconcile")?,
            acquire: export("acquire")?,
        }))
    }
}

#[derive(ComponentType, Lift, Lower)]
#[component(record)]
struct Known {
    pending: bool,
    retired: bool,
    #[component(name = "external-id")]
    external_id: String,
    revision: String,
    metadata: String,
}

#[derive(ComponentType, Lift, Lower)]
#[component(variant)]
enum Decision {
    #[component(name = "unchanged")]
    Unchanged,
    #[component(name = "refresh")]
    Refresh(String),
    #[component(name = "retire")]
    Retire(String),
}

#[derive(ComponentType, Lift, Lower)]
#[component(variant)]
enum Acquired {
    #[component(name = "document")]
    Document(wit::RawDocument),
    #[component(name = "skip")]
    Skip,
    #[component(name = "retire")]
    Retire(String),
}

#[derive(ComponentType, Lift, Lower)]
#[component(record)]
struct Page {
    candidates: Vec<wit::Candidate>,
    exhausted: bool,
    #[component(name = "cursor-after")]
    cursor_after: Option<String>,
}

type Answer<T> = (std::result::Result<T, wit::SourceError>,);

impl Inner {
    pub(super) async fn validate_reconciliation(&self) -> Result<()> {
        let Some(extension) = &self.reconciliation else {
            return Ok(());
        };
        let pre = self.raw_pre.clone();
        let (reconcile, discover, acquire) =
            (extension.reconcile, extension.discover, extension.acquire);
        self.invoke(false, None, move |store| {
            Box::pin(async move {
                let instance = pre.instantiate_async(&mut *store).await?;
                instance.get_typed_func::<(wit::SourceRef, Known), Answer<Decision>>(
                    &mut *store,
                    &reconcile,
                )?;
                instance.get_typed_func::<(wit::SourceRef, String, u32), Answer<Page>>(
                    &mut *store,
                    &discover,
                )?;
                instance.get_typed_func::<(wit::SourceRef, wit::Candidate), Answer<Acquired>>(
                    &mut *store,
                    &acquire,
                )?;
                Ok(Ok(()))
            })
        })
        .await
    }

    pub(super) async fn reconcile_document(
        &self,
        source: &SourceRef,
        known: &KnownDocument,
    ) -> Result<Reconciliation> {
        let Some(extension) = &self.reconciliation else {
            return Ok(Reconciliation::Unchanged);
        };
        let pre = self.raw_pre.clone();
        let index = extension.reconcile;
        let argument = to_wit(source);
        let known = Known {
            pending: known.pending,
            retired: known.retired,
            external_id: known.external_id.clone(),
            revision: known.revision.clone(),
            metadata: known.metadata.to_string(),
        };
        let decision = self
            .invoke(true, Some(&source.locator), move |store| {
                Box::pin(async move {
                    let instance = pre.instantiate_async(&mut *store).await?;
                    let function = instance
                        .get_typed_func::<(wit::SourceRef, Known), Answer<Decision>>(
                            &mut *store,
                            &index,
                        )?;
                    // Concurrent calls perform post-return as part of task completion.
                    store
                        .run_concurrent(async move |access| {
                            let (result,) =
                                function.call_concurrent(access, (argument, known)).await?;
                            Ok(result)
                        })
                        .await?
                })
            })
            .await?;
        Ok(match decision {
            Decision::Unchanged => Reconciliation::Unchanged,
            Decision::Refresh(handle) => Reconciliation::Refresh(handle),
            Decision::Retire(reason) => Reconciliation::Retire(reason),
        })
    }

    pub(super) async fn discover_reconciled(
        &self,
        source: &SourceRef,
        cursor: &Cursor,
        limit: usize,
    ) -> Result<Discovery> {
        let extension = self
            .reconciliation
            .as_ref()
            .expect("only called for a reconciling source");
        let pre = self.raw_pre.clone();
        let index = extension.discover;
        let args = (
            to_wit(source),
            cursor.to_string(),
            u32::try_from(limit).unwrap_or(u32::MAX),
        );
        let page = self
            .invoke(true, Some(&source.locator), move |store| {
                Box::pin(async move {
                    let instance = pre.instantiate_async(&mut *store).await?;
                    let function = instance
                        .get_typed_func::<(wit::SourceRef, String, u32), Answer<Page>>(
                            &mut *store,
                            &index,
                        )?;
                    store
                        .run_concurrent(async move |access| {
                            let (result,) = function.call_concurrent(access, args).await?;
                            Ok(result)
                        })
                        .await?
                })
            })
            .await?;
        let parse_cursor = |value: &str| {
            serde_json::from_str(value)
                .map_err(|_| self.failed("source returned a non-JSON page checkpoint"))
        };
        Ok(Discovery {
            candidates: page
                .candidates
                .into_iter()
                .map(|candidate| {
                    Ok(Candidate {
                        external_id: candidate.external_id,
                        handle: candidate.handle,
                        cursor_after: parse_cursor(&candidate.cursor_after)?,
                    })
                })
                .collect::<Result<_>>()?,
            exhausted: page.exhausted,
            cursor_after: page.cursor_after.as_deref().map(parse_cursor).transpose()?,
            filtered: 0,
        })
    }

    pub(super) async fn acquire_document(
        &self,
        source: &SourceRef,
        candidate: &Candidate,
    ) -> Result<Acquisition> {
        let extension = self
            .reconciliation
            .as_ref()
            .expect("only called for a reconciling source");
        let pre = self.raw_pre.clone();
        let index = extension.acquire;
        let args = (
            to_wit(source),
            wit::Candidate {
                external_id: candidate.external_id.clone(),
                handle: candidate.handle.clone(),
                cursor_after: candidate.cursor_after.to_string(),
            },
        );
        let acquired = self
            .invoke(true, Some(&source.locator), move |store| {
                Box::pin(async move {
                    let instance = pre.instantiate_async(&mut *store).await?;
                    let function = instance
                        .get_typed_func::<(wit::SourceRef, wit::Candidate), Answer<Acquired>>(
                            &mut *store,
                            &index,
                        )?;
                    store
                        .run_concurrent(async move |access| {
                            let (result,) = function.call_concurrent(access, args).await?;
                            Ok(result)
                        })
                        .await?
                })
            })
            .await?;
        Ok(match acquired {
            Acquired::Skip => Acquisition::Skip,
            Acquired::Retire(reason) => Acquisition::Retire(reason),
            Acquired::Document(raw) => {
                let metadata = serde_json::from_str(&raw.metadata)
                    .map_err(|_| self.failed("source returned non-JSON metadata"))?;
                let occurred_at = raw
                    .occurred_at
                    .map(|at| {
                        chrono::DateTime::parse_from_rfc3339(&at)
                            .map(|at| at.with_timezone(&chrono::Utc))
                            .map_err(|_| self.failed("source returned an invalid event timestamp"))
                    })
                    .transpose()?;
                Acquisition::Document(crate::domain::RawDocument {
                    external_id: raw.external_id,
                    revision: raw.revision,
                    body: raw.body,
                    metadata,
                    occurred_at,
                })
            }
        })
    }
}
