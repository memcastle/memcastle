//! The contract between a source and the mining pipeline.
//!
//! An adapter knows one thing: how to find and read documents of one kind of source, and how to put them in
//! MemCastle's terms. It does not know about jobs, drawers, chunks, deduplication, the store or the scheduler: the
//! pipeline (`pipeline.rs`) owns all of that, identically for every adapter. That split is what lets a Slack or
//! GitHub adapter be added without touching how mining persists, resumes or deduplicates.
//!
//! The stages an adapter takes part in:
//!
//! 1. [`SourceAdapter::discover`]: cheap, ordered, resumable from a [`Cursor`]; yields [`Candidate`]s, not bodies.
//! 2. [`SourceAdapter::read`]: acquire one candidate's [`RawDocument`]. No model, no agent.
//! 3. [`SourceAdapter::normalize`]: pure, no I/O: raw to [`CanonicalDocument`].
//!
//! Chunking, filing and the cursor commit follow in the pipeline. Anything semantic (entity extraction,
//! summarisation) comes after, as a separate stage over what was filed.

use std::future::Future;

use crate::domain::{
    Candidate, CanonicalDocument, Cursor, Options, RawDocument, SourceCapabilities, SourceRef,
};
use crate::error::Result;

/// What discovery found past a cursor.
#[derive(Debug, Default)]
pub struct Discovery {
    /// Candidates in cursor order, at most the requested limit. Each carries the cursor to store once it and all
    /// before it are done.
    pub candidates: Vec<Candidate>,
    /// Whether nothing is left past the last candidate. `false` means a further run would find more.
    pub exhausted: bool,
    /// Candidates excluded before paging by the adapter's own filters, when it can count them (zero otherwise).
    pub filtered: usize,
    /// Optional page checkpoint, committed only after every candidate succeeded.
    /// Also permits empty pages to advance without inventing a document.
    pub cursor_after: Option<Cursor>,
}

pub use crate::domain::KnownDocument;

/// A provider's explicit decision about one previously ingested document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconciliation {
    /// The document remains current; no body is needed.
    Unchanged,
    /// Acquire it again using this opaque handle and the known external ID.
    Refresh(String),
    /// Confirmed deletion, lost sharing or scope exclusion, not a transient error.
    Retire(String),
}

/// Acquisition distinguishes an intentional skip from confirmed removal.
pub enum Acquisition {
    /// A live document to normalize and ingest.
    Document(crate::domain::RawDocument),
    /// Nothing to ingest; existing content is left current.
    Skip,
    /// Retire this candidate's previously ingested chunks, preserving history.
    Retire(String),
}

/// One kind of source. Implementations are stateless apart from configuration, and every method may run on any
/// run of any job.
///
/// Dispatch is static (`mining::run` resolves the source to a `registry::AnySource` and calls the generic
/// pipeline), so the async methods are plain `impl Future` returns, with `Send` spelled out because jobs run on a
/// multi-threaded runtime.
///
/// This is the one logical contract every source meets, whether it is compiled into MemCastle or is a WebAssembly
/// component loaded at run time (docs/adr/026); the strings it returns are borrowed from the adapter rather than
/// `'static` so that a loaded source can own its name.
pub trait SourceAdapter: Send + Sync {
    /// Opt into Core-owned inventory reconciliation before discovery, including full runs.
    fn reconciles(&self) -> bool {
        false
    }

    /// Recheck a bounded inventory item using the run's current scope and credential.
    fn reconcile(
        &self,
        _source: &SourceRef,
        _known: &KnownDocument,
    ) -> impl Future<Output = Result<Reconciliation>> + Send {
        async { Ok(Reconciliation::Unchanged) }
    }

    /// Explicit acquisition outcome; legacy adapters keep their skip semantics.
    fn acquire(
        &self,
        source: &SourceRef,
        candidate: &Candidate,
    ) -> impl Future<Output = Result<Acquisition>> + Send {
        async {
            Ok(self
                .read(source, candidate)
                .await?
                .map_or(Acquisition::Skip, Acquisition::Document))
        }
    }
    /// The adapter's name, as users give it (`memcastle mine <name>`) and as it is stored on the source.
    fn name(&self) -> &str;

    /// One line saying what the adapter reads, for `memcastle sources` and the API.
    fn description(&self) -> &str;

    /// What this adapter can do.
    fn capabilities(&self) -> SourceCapabilities;

    /// The source `locator` names, or the adapter's default when `None`, checked to exist.
    ///
    /// The one place a locator and the run's `options` are validated and normalised (a path made canonical, a date
    /// parsed), so that two spellings of the same place become the same source and share a cursor. The returned
    /// [`SourceRef`] carries the normalised options on to `discover` and `read`.
    ///
    /// A filter that selects a different slice of the same place (a `dir` within a history) must be folded into the
    /// source's `account` or `locator` here, so each slice keeps a cursor of its own; a filter that only narrows
    /// (`since`) must not be, or every value would re-read everything (docs/adr/041).
    ///
    /// # Errors
    ///
    /// An error when the locator or an option is not valid for this adapter or what it names cannot be reached.
    fn identify(&self, locator: Option<&str>, options: &Options) -> Result<SourceRef>;

    /// The wing drawers are filed under when the caller does not choose one.
    fn default_wing(&self, source: &SourceRef) -> String;

    /// The room drawers are filed under when a document does not name one.
    fn default_room(&self) -> &str;

    /// Candidates strictly after `cursor` (`null` is the beginning), in cursor order, at most `limit`.
    ///
    /// Must be cheap relative to [`SourceAdapter::read`]: a listing, not a download.
    ///
    /// # Errors
    ///
    /// [`crate::Error::SourceCursorInvalid`] when `cursor` is not one this adapter produced, or an I/O or source
    /// error when the listing fails.
    fn discover(
        &self,
        source: &SourceRef,
        cursor: &Cursor,
        limit: usize,
    ) -> impl Future<Output = Result<Discovery>> + Send;

    /// Read `candidate`, or `None` when it should be skipped (gone since discovery, binary, too large). Skipping
    /// is not an error: a tree of mixed files makes it the normal case.
    ///
    /// # Errors
    ///
    /// An I/O or source error that should fail the job.
    fn read(
        &self,
        source: &SourceRef,
        candidate: &Candidate,
    ) -> impl Future<Output = Result<Option<RawDocument>>> + Send;

    /// Express `raw` as a canonical document. Pure: it must not touch the outside world, so that normalisation can
    /// be tested from fixtures and re-run from a stored raw document.
    ///
    /// A document with nothing worth filing returns no segments; the pipeline then files no drawers for it but
    /// still remembers its revision.
    ///
    /// # Errors
    ///
    /// An error when `raw` is malformed beyond what can be skipped.
    fn normalize(&self, raw: &RawDocument) -> Result<CanonicalDocument>;
}
