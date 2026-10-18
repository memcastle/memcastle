//! Miner configuration: global `[[miners]]` and per-miner TOML files, listed, changed and run.
//!
//! One definition per miner, read back from its own file or the daemon's global file, never held in a
//! second datastore (`docs/adr/037-persistent-miner-configuration.md`). The CLI, REST and read-only MCP tools all call
//! the functions here, so there is one set of rules.
//!
//! Changing miners is administrative like installing a source: it is not gated by the memory mode, and no MCP tool
//! does it, so an agent can read the miners but cannot configure what the daemon mines. Running one is a memory
//! write, because it files drawers, and is gated as one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::AppServices;
use crate::config::miners_file::{self, FileStamp};
use crate::domain::{
    CredentialRef, Job, JobKind, JobStatus, MemoryMode, MinerDefinition, MinerMapping,
    MinerSchedule, MiningSource, NameKind, SourceId, SourceRecord, option_broadening,
    validate_name,
};
use crate::error::{Error, Result};

/// Where the miners are, and the last good copy of them.
///
/// The file is the truth; this holds what was last read from it so a hand edit is noticed (the file's stamp changed),
/// an invalid one does not take the miners away (the last good copy stays, with the error beside it), and a write
/// can refuse to overwrite an edit it has not seen.
pub struct MinerRegistry {
    /// The configuration file. `None` for a configuration built in code, where changing miners is refused.
    path: Option<PathBuf>,
    default_dir: Option<PathBuf>,
    extra_dirs: Vec<PathBuf>,
    state: Mutex<RegistryState>,
}

struct RegistryState {
    /// The last miners that parsed and validated.
    miners: Vec<MinerDefinition>,
    /// Origin for each loaded ID; `None` is the global `[[miners]]` section.
    origins: BTreeMap<String, Option<PathBuf>>,
    file_stamps: BTreeMap<PathBuf, Option<FileStamp>>,
    /// What the file looked like when `miners` was read; `None` before the first read.
    stamp: Option<Option<FileStamp>>,
    /// Why the file could not be read the last time it changed.
    error: Option<String>,
}

impl MinerRegistry {
    /// A registry over `path`, starting from the miners the daemon loaded at startup.
    #[must_use]
    pub fn new(path: Option<PathBuf>, initial: Vec<MinerDefinition>) -> Self {
        Self {
            path,
            default_dir: None,
            extra_dirs: Vec::new(),
            state: Mutex::new(RegistryState {
                origins: initial.iter().map(|m| (m.name.clone(), None)).collect(),
                miners: initial,
                file_stamps: BTreeMap::new(),
                stamp: None,
                error: None,
            }),
        }
    }

    /// Add directories after the global file; missing default directory is an empty collection.
    #[must_use]
    pub fn with_directories(
        mut self,
        default_dir: Option<PathBuf>,
        extra_dirs: Vec<PathBuf>,
    ) -> Self {
        self.default_dir = default_dir;
        self.extra_dirs = extra_dirs;
        self
    }

    /// The miners as they are now: a hand edit made since the last look is picked up first, and an invalid file leaves
    /// the last good copy. What another service (the triggers) reads, so it never reaches into the registry's state.
    pub(super) async fn snapshot(&self) -> Vec<MinerDefinition> {
        let mut state = self.state.lock().await;
        // The error is in `state.error`; a failed refresh must not turn a read into a failure.
        let _ = self.refresh(&mut state, false);
        state.miners.clone()
    }

    /// The path, or the error that says changing miners needs one.
    fn path(&self) -> Result<&Path> {
        self.path.as_deref().ok_or_else(|| Error::MinerConfigFile {
            path: "-".to_string(),
            reason: "this daemon was not started from a configuration file, so there is nowhere to keep miners"
                .to_string(),
        })
    }

    /// Read the file again if it changed (or `force`), keeping the last good copy when it no longer parses.
    ///
    /// Returns what changed. A hand edit that may broaden options is applied because the file is the user's, but is
    /// logged; without source metadata in this synchronous path the reload report is deliberately conservative.
    fn refresh(&self, state: &mut RegistryState, force: bool) -> Result<MinersDiff> {
        let Some(path) = self.path.as_deref() else {
            return Ok(MinersDiff::default());
        };
        let current = miners_file::stamp(path).inspect_err(|e| state.error = Some(problem(e)))?;
        if !force
            && self.default_dir.is_none()
            && self.extra_dirs.is_empty()
            && state.stamp == Some(current)
        {
            return Ok(MinersDiff::default());
        }
        // A bad file preserves only its own last-good miner; other files can still be changed or run.
        let mut errors = Vec::new();
        let (global, stamp) = match miners_file::read(path) {
            Ok(loaded) => (loaded.miners, loaded.stamp),
            Err(error) => {
                if self.default_dir.is_none() && self.extra_dirs.is_empty() {
                    state.stamp = Some(current);
                    state.error = Some(problem(&error));
                    return Err(error);
                }
                errors.push(format!("{}: {}", path.display(), problem(&error)));
                (
                    state
                        .miners
                        .iter()
                        .filter(|m| state.origins.get(&m.name) == Some(&None))
                        .cloned()
                        .collect(),
                    current,
                )
            }
        };
        let mut loaded = Vec::new();
        let mut origins: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
        let mut file_stamps = BTreeMap::new();
        for miner in global {
            origins.insert(miner.name.clone(), None);
            loaded.push(miner);
        }
        for (dir, optional) in self
            .default_dir
            .iter()
            .map(|dir| (dir, true))
            .chain(self.extra_dirs.iter().map(|dir| (dir, false)))
        {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => {
                    errors.push(format!(
                        "{}: {error}; check mining.miner_dirs",
                        dir.display()
                    ));
                    continue;
                }
            };
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|entry| match entry {
                    Ok(entry) if entry.path().extension().is_some_and(|ext| ext == "toml") => {
                        Some(entry.path())
                    }
                    Ok(_) => None,
                    Err(error) => {
                        errors.push(format!("{}: {error}", dir.display()));
                        None
                    }
                })
                .collect();
            paths.sort();
            for file in paths {
                let miner = match miners_file::read_one(&file) {
                    Ok((miner, stamp)) => {
                        file_stamps.insert(file.clone(), stamp);
                        miner
                    }
                    Err(error) => {
                        errors.push(format!("{}: {}", file.display(), problem(&error)));
                        let Some(previous) = state
                            .miners
                            .iter()
                            .find(|m| state.origins.get(&m.name) == Some(&Some(file.clone())))
                        else {
                            continue;
                        };
                        previous.clone()
                    }
                };
                if let Some(previous) = origins.get(&miner.name) {
                    let first = previous.as_deref().unwrap_or(path);
                    errors.push(format!(
                        "miner `{}` is defined in both {} and {}; rename one",
                        miner.name,
                        first.display(),
                        file.display()
                    ));
                    continue;
                }
                origins.insert(miner.name.clone(), Some(file));
                loaded.push(miner);
            }
        }
        {
            let diff = MinersDiff::between(&state.miners, &loaded);
            for name in &diff.broadened {
                warn!(miner = %name, "the configuration file may have broadened this miner's options; applied as written");
            }
            if !diff.is_empty() {
                info!(
                    added = ?diff.added, removed = ?diff.removed, changed = ?diff.changed,
                    "miner configuration reloaded"
                );
            }
            state.miners = loaded;
            state.origins = origins;
            state.file_stamps = file_stamps;
            state.stamp = Some(stamp);
            state.error = (!errors.is_empty()).then(|| errors.join("; "));
            Ok(diff)
        }
    }
}

impl Default for MinerRegistry {
    fn default() -> Self {
        Self::new(None, Vec::new())
    }
}

/// The reason inside a configuration-file error, without the sentence around it.
fn problem(error: &Error) -> String {
    match error {
        Error::MinerConfigFile { reason, .. } => reason.clone(),
        other => other.to_string(),
    }
}

/// What a reload found different, by miner name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinersDiff {
    /// Miners that were not there before.
    pub added: Vec<String>,
    /// Miners that are gone.
    pub removed: Vec<String>,
    /// Miners whose definition differs.
    pub changed: Vec<String>,
    /// Changed miners that are now enabled and were not.
    pub enabled: Vec<String>,
    /// Changed miners that are now disabled and were not.
    pub disabled: Vec<String>,
    /// Changed miners whose options may now select more material.
    pub broadened: Vec<String>,
}

impl MinersDiff {
    fn between(old: &[MinerDefinition], new: &[MinerDefinition]) -> Self {
        let mut diff = Self::default();
        for miner in new {
            let Some(before) = old.iter().find(|m| m.name == miner.name) else {
                diff.added.push(miner.name.clone());
                continue;
            };
            if before == miner {
                continue;
            }
            diff.changed.push(miner.name.clone());
            match (before.enabled, miner.enabled) {
                (false, true) => diff.enabled.push(miner.name.clone()),
                (true, false) => diff.disabled.push(miner.name.clone()),
                _ => {}
            }
            // A file reload has no source metadata in this synchronous path: unknown changes are reported conservatively.
            if before.source != miner.source
                || before.locator != miner.locator
                || before.source_config != miner.source_config
                || before.mapping != miner.mapping
                || !option_broadening(
                    &before.options().unwrap_or_default(),
                    &miner.options().unwrap_or_default(),
                    &[],
                )
                .is_empty()
            {
                diff.broadened.push(miner.name.clone());
            }
        }
        diff.removed = old
            .iter()
            .filter(|m| !new.iter().any(|n| n.name == m.name))
            .map(|m| m.name.clone())
            .collect();
        diff
    }

    /// Whether nothing changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// Whether a miner can run right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinerState {
    /// Enabled and its source and credential are usable.
    Ready,
    /// Switched off.
    Disabled,
    /// Enabled, but something it needs is missing; the reason says what.
    Unavailable,
}

/// A miner's credential reference, without the reference: what kind it is and whether it resolves.
///
/// The variable's name or the file's path is not given back, so a client that can read miners learns nothing about
/// where the daemon keeps its secrets, and the value is never read for this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialView {
    /// `env` or `file`.
    pub kind: String,
    /// Whether the variable is set to something, or the file exists.
    pub available: bool,
}

/// One miner as the CLI, REST and MCP show it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinerView {
    /// The miner's name.
    pub name: String,
    /// The source adapter it runs.
    pub source: String,
    /// Whether it may run.
    pub enabled: bool,
    /// Whether it can run right now.
    pub state: MinerState,
    /// Why it cannot, when `state` is `unavailable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The part of the source it reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// The wing its drawers go to, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wing: Option<String>,
    /// Optional profile overriding the global embedding default for this miner's drawers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_profile: Option<String>,
    /// Its credential, as a kind and an availability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<CredentialView>,
    /// Saved source options.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub options: Map<String, Value>,
    /// The source configuration selected, or the source's default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_config: Option<String>,
    /// A fixed room override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    /// Dynamic Wing/Room mapping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<MinerMapping>,
    /// Maximum documents for one job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_documents: Option<usize>,
    /// The miner's optional schedule; off until explicitly enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<MinerSchedule>,
    /// The file this miner is edited in; absent for an entry in the global config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_file: Option<String>,
    /// The mined source this miner's cursor lives on, once something has been mined from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<SourceId>,
    /// When that source was last mined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<DateTime<Utc>>,
    /// How many documents that source holds.
    #[serde(default)]
    pub documents: u64,
}

/// Every miner, and whether the file they come from is currently readable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinersReport {
    /// The miners, in file order.
    pub miners: Vec<MinerView>,
    /// The file they are kept in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_file: Option<String>,
    /// Why the file cannot be read, when it cannot: `miners` is then the last good copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A change to one miner: what to set, and what to clear.
///
/// A key that is absent is left alone. `options` sets the keys it names (replacing each key's value);
/// the `unset_*` lists remove keys. Creating a miner needs `source`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MinerPatch {
    /// The source adapter.
    pub source: Option<String>,
    /// A named configuration of the selected source.
    pub source_config: Option<String>,
    /// Switch it on or off.
    pub enabled: Option<bool>,
    /// The part of the source to read.
    pub locator: Option<String>,
    /// The wing for its drawers.
    pub wing: Option<String>,
    /// A fixed destination room.
    pub room: Option<String>,
    /// Dynamic Wing/Room mapping.
    pub mapping: Option<MinerMapping>,
    /// Per-run document limit.
    pub max_documents: Option<usize>,
    /// An embedding profile supplied by the profile subsystem.
    pub embedding_profile: Option<String>,
    /// Optional timetable, disabled until explicitly enabled.
    pub schedule: Option<MinerSchedule>,
    /// Where its credential comes from.
    pub credential: Option<CredentialRef>,
    /// Source options to set.
    pub options: Map<String, Value>,
    /// Source options to remove.
    pub unset_options: Vec<String>,
    /// Fields to clear: any of `locator`, `wing`, `credential`.
    pub unset: Vec<String>,
    /// Allow a change that may broaden what the miner reads.
    pub allow_broaden: bool,
}

/// One-off changes to a run, carried together so triggered runs can use the empty default.
#[derive(Default)]
struct RunOverrides {
    options: crate::domain::Options,
    allow_broaden: bool,
}

/// The outcome of [`AppServices::set_miner`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinerChange {
    /// The miner as it is now.
    pub miner: MinerView,
    /// Whether it did not exist before.
    pub created: bool,
    /// Whether anything was written; false when the request changed nothing.
    pub changed: bool,
    /// Whether it now reads a different source or locator, which is a different cursor.
    pub identity_changed: bool,
}

/// The outcome of [`AppServices::reload_miners`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinersReload {
    /// How many miners are configured now.
    pub miners: usize,
    /// What differs from what the daemon had.
    #[serde(flatten)]
    pub diff: MinersDiff,
    /// Invalid files leave their last good definitions in place; name every affected file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AppServices {
    /// Give the services the miner registry the daemon built from its configuration file.
    #[must_use]
    pub fn with_miners(mut self, miners: MinerRegistry) -> Self {
        self.miners = Arc::new(miners);
        self
    }

    /// Every configured miner with its state.
    ///
    /// Gated as a **read**. A hand edit made since the last call is picked up first; if the file no longer parses
    /// the last good miners are reported with the error beside them, not an error, so an agent or a dashboard can
    /// still see what is configured and why it is stale.
    ///
    /// # Errors
    ///
    /// [`Error::ModeForbidden`] if `mode` forbids reads, or a store error.
    pub async fn list_miners(&self, mode: MemoryMode) -> Result<MinersReport> {
        Self::require_read(mode, "miner_list")?;
        let (miners, origins, error) = {
            let mut state = self.miners.state.lock().await;
            // The error is in `state.error`; a failed refresh must not turn a read into a failure.
            let _ = self.miners.refresh(&mut state, false);
            (
                state.miners.clone(),
                state.origins.clone(),
                state.error.clone(),
            )
        };
        let sources = self.store.list_sources().await?;
        let mut views = Vec::with_capacity(miners.len());
        for miner in &miners {
            let mut view = self.miner_view(miner, &sources).await?;
            view.config_file = origins
                .get(&miner.name)
                .and_then(|p| p.as_ref())
                .map(|p| p.display().to_string());
            views.push(view);
        }
        Ok(MinersReport {
            miners: views,
            config_file: self.miners.path.as_ref().map(|p| p.display().to_string()),
            error,
        })
    }

    /// One miner.
    ///
    /// # Errors
    ///
    /// [`Error::MinerNotFound`], [`Error::ModeForbidden`] if `mode` forbids reads, or a store error.
    pub async fn show_miner(&self, name: &str, mode: MemoryMode) -> Result<MinerView> {
        Self::require_read(mode, "miner_get")?;
        let (miner, origin) = {
            let mut state = self.miners.state.lock().await;
            let _ = self.miners.refresh(&mut state, false);
            (
                find(&state.miners, name)?.clone(),
                state.origins.get(name).cloned().flatten(),
            )
        };
        let sources = self.store.list_sources().await?;
        let mut view = self.miner_view(&miner, &sources).await?;
        view.config_file = origin.map(|p| p.display().to_string());
        Ok(view)
    }

    /// Create a miner, or change one: the same call for both, so `miner set` is idempotent.
    ///
    /// Everything is checked before anything is written: the definition's shape, the wing's name, that an enabled
    /// miner's source is usable and its credential resolves, and that the change does not widen the scope (unless
    /// `allow_broaden`). A rejected change leaves the file untouched.
    ///
    /// Administrative: not gated by the memory mode, and not reachable from MCP.
    ///
    /// # Errors
    ///
    /// [`Error::MinerInvalid`], [`Error::MinerScopeBroadened`], [`Error::MinerConfigFile`] when the file is
    /// unreadable or changed underneath, and store errors.
    pub async fn set_miner(&self, name: &str, patch: MinerPatch) -> Result<MinerChange> {
        self.apply_patch(name, patch, false).await
    }

    /// Switch a miner on or off. Idempotent. Switching on checks that it can run, like [`Self::set_miner`].
    ///
    /// # Errors
    ///
    /// [`Error::MinerNotFound`] and the errors of [`Self::set_miner`].
    pub async fn set_miner_enabled(&self, name: &str, enabled: bool) -> Result<MinerChange> {
        let patch = MinerPatch {
            enabled: Some(enabled),
            ..MinerPatch::default()
        };
        self.apply_patch(name, patch, true).await
    }

    /// Enable or disable only this miner's timetable; creating it never starts it.
    pub async fn set_miner_schedule_enabled(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<MinerChange> {
        let miner = {
            let mut state = self.miners.state.lock().await;
            self.miners.refresh(&mut state, false)?;
            find(&state.miners, name)?.clone()
        };
        let Some(mut schedule) = miner.schedule else {
            return Err(invalid(
                name,
                "this miner has no schedule; set `schedule.every` first",
            ));
        };
        if enabled && !miner.enabled {
            return Err(invalid(
                name,
                "enable the miner before enabling its schedule",
            ));
        }
        schedule.enabled = enabled;
        self.apply_patch(
            name,
            MinerPatch {
                schedule: Some(schedule),
                ..MinerPatch::default()
            },
            true,
        )
        .await
    }

    /// Remove a miner's definition. What it mined, and the source's cursor, stay.
    ///
    /// # Errors
    ///
    /// [`Error::MinerNotFound`] and [`Error::MinerConfigFile`].
    pub async fn remove_miner(&self, name: &str) -> Result<()> {
        let path = self.miners.path()?;
        let mut state = self.miners.state.lock().await;
        self.refresh_for_write(&mut state)?;
        find(&state.miners, name)?;
        let remaining: Vec<MinerDefinition> = state
            .miners
            .iter()
            .filter(|m| m.name != name)
            .cloned()
            .collect();
        if let Some(Some(file)) = state.origins.get(name) {
            miners_file::write_one(file, state.file_stamps.get(file).copied().flatten(), None)?;
        } else {
            let global: Vec<_> = remaining
                .iter()
                .filter(|m| state.origins.get(&m.name) == Some(&None))
                .cloned()
                .collect();
            state.stamp = Some(miners_file::write(path, state.stamp.flatten(), &global)?);
        }
        self.miners.refresh(&mut state, true)?;
        info!(miner = %name, "miner removed");
        Ok(())
    }

    /// Read the configuration file again now, whether or not it looks changed, and say what differs.
    ///
    /// # Errors
    ///
    /// [`Error::MinerConfigFile`] when the file cannot be read or does not validate; the daemon keeps the last good
    /// miners.
    pub async fn reload_miners(&self) -> Result<MinersReload> {
        self.miners.path()?;
        let mut state = self.miners.state.lock().await;
        let diff = self.miners.refresh(&mut state, true)?;
        Ok(MinersReload {
            miners: state.miners.len(),
            diff,
            error: state.error.clone(),
        })
    }

    /// Submit the mining job for a miner.
    ///
    /// Refused when the miner is disabled or cannot run. The miner's scope and settings are passed to the source as the
    /// run's options, exactly as `memcastle mine <source> key=value` would; a key the source does not declare makes the
    /// miner not runnable rather than being ignored. The job mines the miner's source and locator, so it continues from
    /// the cursor the same source already has.
    ///
    /// Gated as a **write**, like any mining.
    ///
    /// # Errors
    ///
    /// [`Error::MinerNotFound`], [`Error::MinerDisabled`], [`Error::MinerNotRunnable`], and the errors of
    /// [`AppServices::submit_mine`].
    pub async fn run_miner(
        &self,
        name: &str,
        full: bool,
        requested_by: &str,
        mode: MemoryMode,
    ) -> Result<Job> {
        Self::require_write(mode, "miner_run")?;
        self.request_miner_run(name, full, requested_by, mode, false)
            .await
            .map(|(job, _)| job)
    }

    /// Run with temporary source options, leaving the miner's saved definition intact.
    pub async fn run_miner_with_options(
        &self,
        name: &str,
        full: bool,
        overrides: crate::domain::Options,
        allow_broaden: bool,
        requested_by: &str,
        mode: MemoryMode,
    ) -> Result<Job> {
        Self::require_write(mode, "miner_run")?;
        self.request_miner_run_with_options(
            name,
            full,
            requested_by,
            mode,
            false,
            RunOverrides {
                options: overrides,
                allow_broaden,
            },
        )
        .await
        .map(|(job, _)| job)
    }

    /// Ask for a run of a miner: the one path every request for a run takes, whether a person made it
    /// (`memcastle miner run`) or a trigger did (docs/adr/043), so a trigger can cause nothing a person could not.
    ///
    /// With `join_waiting`, a run of the same source with the same settings that is still queued is returned instead
    /// of queueing another, and the second value says so: that run will read whatever has changed by the time it
    /// starts, so a burst of requests need not become a burst of jobs. A run that is already *running* does not
    /// count, because it may have passed the part that changed.
    ///
    /// Does not check the memory mode's write gate itself: the caller has, or is a trigger the user set up, which is
    /// not a client session.
    pub(super) async fn request_miner_run(
        &self,
        name: &str,
        full: bool,
        requested_by: &str,
        mode: MemoryMode,
        join_waiting: bool,
    ) -> Result<(Job, bool)> {
        self.request_miner_run_with_options(
            name,
            full,
            requested_by,
            mode,
            join_waiting,
            RunOverrides::default(),
        )
        .await
    }

    async fn request_miner_run_with_options(
        &self,
        name: &str,
        full: bool,
        requested_by: &str,
        mode: MemoryMode,
        join_waiting: bool,
        overrides: RunOverrides,
    ) -> Result<(Job, bool)> {
        let miner = {
            let mut state = self.miners.state.lock().await;
            let _ = self.miners.refresh(&mut state, false);
            find(&state.miners, name)?.clone()
        };
        if !miner.enabled {
            return Err(Error::MinerDisabled { name: miner.name });
        }
        let not_runnable = |reason: String| Error::MinerNotRunnable {
            name: miner.name.clone(),
            reason,
        };
        if let Err(reason) = self.check_activation(&miner).await {
            return Err(not_runnable(reason));
        }
        // The saved options are the run's options; `check_activation` has already checked their keys against
        // what the source declares, so what reaches the source is what the miner says.
        let configuration = self
            .source_config(
                miner.source_config.as_deref().unwrap_or(&miner.source),
                Some(&miner.source),
            )
            .await
            .map_err(|e| not_runnable(e.to_string()))?;
        let mut saved = configuration.options().map_err(not_runnable)?;
        saved.extend(miner.options().map_err(not_runnable)?);
        let mut options = saved.clone();
        options.extend(overrides.options);
        let mut checked = miner.clone();
        checked.options = options
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect();
        checked.validate().map_err(not_runnable)?;
        crate::mining::registry::ensure_minable(&self.store, &self.mining, &miner.source, &options)
            .await?;
        // Unknown options, including unknown filtering semantics, need an explicit one-off approval.
        let specs =
            crate::mining::registry::option_specs_for(&self.store, &self.mining, &miner.source)
                .await?;
        let reasons = option_broadening(&saved, &options, &specs);
        if !overrides.allow_broaden && !reasons.is_empty() {
            return Err(Error::MinerScopeBroadened {
                name: name.to_string(),
                reasons: reasons.join("; "),
            });
        }
        let preset = crate::domain::MiningPreset {
            miner: Some(miner.name.clone()),
            configuration: Some(configuration.id),
            room: miner.room.clone(),
            mapping: miner.mapping.clone(),
            max_documents: Some(miner.max_documents.unwrap_or(self.mining.max_documents)),
            embedding_profile: miner.embedding_profile.clone(),
        };
        if join_waiting && !full {
            let queued = self.store.list_jobs(Some(JobStatus::Queued)).await?;
            let waiting = queued.into_iter().find(|job| match &job.kind {
                JobKind::Mine {
                    source,
                    wing,
                    options: waiting_options,
                    preset: waiting_preset,
                    ..
                } => {
                    let same_source = match source {
                        MiningSource::Named { source, locator } => {
                            *source == miner.source && *locator == miner.locator
                        }
                        // The older wire form of a `directory` job.
                        MiningSource::Directory { path } => {
                            miner.source == "directory"
                                && miner.locator.as_deref() == Some(&*path.to_string_lossy())
                        }
                    };
                    same_source
                        && *wing == miner.wing
                        && *waiting_options == options
                        && waiting_preset.as_deref() == Some(&preset)
                }
                _ => false,
            });
            if let Some(job) = waiting {
                return Ok((job, true));
            }
        }
        let job = self
            .submit_mine_preset(
                MiningSource::Named {
                    source: miner.source.clone(),
                    // A source may need none (`opencode` reads its own history); the ones that do refuse it at activation.
                    locator: miner.locator.clone(),
                },
                miner.wing.clone(),
                full,
                options,
                Some(preset),
                requested_by,
                mode,
            )
            .await?;
        Ok((job, false))
    }

    /// Re-read the file for a write, and refuse when it cannot be trusted.
    fn refresh_for_write(&self, state: &mut RegistryState) -> Result<()> {
        // A hand edit since the last request is applied first, so the write is built on what is in the file.
        self.miners.refresh(state, false)?;
        if let Some(reason) = &state.error
            && self.miners.default_dir.is_none()
            && self.miners.extra_dirs.is_empty()
        {
            // Writing would overwrite whatever the user was in the middle of fixing.
            return Err(Error::MinerConfigFile {
                path: self.miners.path()?.display().to_string(),
                reason: reason.clone(),
            });
        }
        Ok(())
    }

    async fn apply_patch(
        &self,
        name: &str,
        patch: MinerPatch,
        must_exist: bool,
    ) -> Result<MinerChange> {
        let path = self.miners.path()?;
        let mut state = self.miners.state.lock().await;
        self.refresh_for_write(&mut state)?;

        let existing = state.miners.iter().find(|m| m.name == name).cloned();
        if must_exist && existing.is_none() {
            return Err(Error::MinerNotFound {
                name: name.to_string(),
            });
        }
        let allow_broaden = patch.allow_broaden;
        let candidate = apply(name, existing.as_ref(), patch)?;
        if let Some(wing) = &candidate.wing {
            validate_name(NameKind::Wing, wing).map_err(|e| invalid(name, e.to_string()))?;
        }
        if let Some(room) = &candidate.room {
            validate_name(NameKind::Room, room).map_err(|e| invalid(name, e.to_string()))?;
        }
        if let Some(before) = &existing {
            let specs = crate::mining::registry::option_specs_for(
                &self.store,
                &self.mining,
                &before.source,
            )
            .await
            .unwrap_or_default();
            let reasons = if before.source != candidate.source
                || before.locator != candidate.locator
                || before.source_config != candidate.source_config
                || before.mapping != candidate.mapping
            {
                vec!["source, source configuration, locator or mapping changed; the miner may read a different place".to_string()]
            } else {
                option_broadening(
                    &before.options().map_err(|r| invalid(name, r))?,
                    &candidate.options().map_err(|r| invalid(name, r))?,
                    &specs,
                )
            };
            if !reasons.is_empty() && !allow_broaden {
                return Err(Error::MinerScopeBroadened {
                    name: name.to_string(),
                    reasons: reasons.join("; "),
                });
            }
        }
        // Only a miner that is going to run is held to what running needs: a disabled one may name a source that is
        // not installed yet, so a configuration can be written ahead of the install.
        if candidate.enabled && existing.as_ref() != Some(&candidate) {
            self.check_activation(&candidate)
                .await
                .map_err(|reason| invalid(name, reason))?;
        }

        let created = existing.is_none();
        let identity_changed = existing.as_ref().is_some_and(|b| {
            b.source != candidate.source
                || b.locator != candidate.locator
                || b.source_config != candidate.source_config
                || b.room != candidate.room
                || b.mapping != candidate.mapping
        });
        let changed = existing.as_ref() != Some(&candidate);
        if changed {
            let mut all = state.miners.clone();
            match all.iter_mut().find(|m| m.name == name) {
                Some(slot) => *slot = candidate.clone(),
                None => all.push(candidate.clone()),
            }
            let file = state.origins.get(name).cloned().flatten().or_else(|| {
                (!state.origins.contains_key(name))
                    .then(|| self.miners.default_dir.clone())
                    .flatten()
                    .map(|dir| dir.join(format!("{name}.toml")))
            });
            if let Some(file) = file {
                let expected = state.file_stamps.get(&file).copied().flatten();
                if state.origins.get(name).and_then(|path| path.as_ref()) == Some(&file)
                    && !state.file_stamps.contains_key(&file)
                {
                    return Err(Error::MinerConfigFile {
                        path: file.display().to_string(),
                        reason: "the file is invalid; fix it and reload before editing this miner"
                            .into(),
                    });
                }
                miners_file::write_one(&file, expected, Some(&candidate))?;
            } else {
                let global: Vec<_> = all
                    .into_iter()
                    .filter(|m| state.origins.get(&m.name) == Some(&None) || m.name == name)
                    .collect();
                state.stamp = Some(miners_file::write(path, state.stamp.flatten(), &global)?);
            }
            self.miners.refresh(&mut state, true)?;
            info!(miner = %name, created, "miner configuration written");
        }
        drop(state);

        let sources = self.store.list_sources().await?;
        Ok(MinerChange {
            miner: self.miner_view(&candidate, &sources).await?,
            created,
            changed,
            identity_changed,
        })
    }

    /// What an enabled miner needs in order to run: a usable source, a locator the source can use, a credential that
    /// resolves. The reason is a sentence that says what to do.
    pub(super) async fn check_activation(
        &self,
        miner: &MinerDefinition,
    ) -> std::result::Result<(), String> {
        if let Some(profile) = &miner.embedding_profile {
            let selected = self
                .embeddings
                .select(Some(profile))
                .map_err(|error| error.to_string())?;
            if !selected.is_configured() {
                return Err(format!(
                    "embedding profile `{profile}` is external or has no provider; choose a runnable profile"
                ));
            }
        }
        if let Some(room) = &miner.room {
            validate_name(NameKind::Room, room).map_err(|error| error.to_string())?;
        }
        // The saved options are what a run passes to its source, so their keys are the source's to accept.
        let config = self
            .source_config(
                miner.source_config.as_deref().unwrap_or(&miner.source),
                Some(&miner.source),
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut options = config.options()?;
        options.extend(miner.options()?);
        crate::mining::registry::ensure_minable(&self.store, &self.mining, &miner.source, &options)
            .await
            .map_err(|e| e.to_string())?;
        if miner.source == "directory" {
            let Some(locator) = miner.locator.as_deref() else {
                return Err(
                    "the directory source needs a `locator`: the absolute path to mine".to_string(),
                );
            };
            let path = Path::new(locator);
            // The daemon resolves a relative path against its own working directory, which is not the caller's.
            if !(path.is_absolute() || path.has_root()) {
                return Err(format!(
                    "the locator `{locator}` is relative; give an absolute path, since the daemon resolves it \
                     against its own working directory"
                ));
            }
        }
        self.check_credential(miner).await?;
        Ok(())
    }

    /// A source that signs in with OAuth needs that done whatever the miner says, and a miner that says `oauth` for a
    /// source that does not is mistaken: both are said here, with the command that fixes it.
    pub(super) async fn check_credential(
        &self,
        miner: &MinerDefinition,
    ) -> std::result::Result<(), String> {
        let requirement = self.oauth_requirement(&miner.source).await;
        let config = self
            .source_config(
                miner.source_config.as_deref().unwrap_or(&miner.source),
                Some(&miner.source),
            )
            .await
            .map_err(|e| e.to_string())?;
        if config.id != miner.source
            && requirement.is_some()
            && miner.locator.is_none()
            && self
                .store
                .get_source_package(&miner.source)
                .await
                .ok()
                .flatten()
                .is_some_and(|record| record.manifest.capabilities.single_account_auth)
        {
            return Err(format!(
                "`{}` supports one default web account; use its default configuration or an offline export locator",
                miner.source
            ));
        }
        let credential = miner.credential.as_ref().or(config.credential.as_ref());
        match credential {
            Some(CredentialRef::Env { name })
                if !credential_available(&CredentialRef::Env { name: name.clone() }) =>
            {
                return Err(format!(
                    "the credential's environment variable `{name}` is not set or is empty in the daemon's \
                     environment; export it and restart the daemon, or disable the miner"
                ));
            }
            Some(CredentialRef::File { path }) if !Path::new(path).is_file() => {
                return Err(format!(
                    "the credential's file `{path}` does not exist; create it, or disable the miner"
                ));
            }
            Some(CredentialRef::Oauth) if requirement.is_none() => {
                return Err(format!(
                    "the source `{}` does not sign in with OAuth, so `oauth` is not a credential it can use; \
                     use `--credential-env` or `--credential-file`, or unset the credential",
                    miner.source
                ));
            }
            _ => {}
        }
        if let Some(requirement) = requirement
            // An on-demand OAuth source may mine offline exports or use its explicitly granted manual-session
            // fallback; an explicit `oauth` miner reference still promises that this particular miner is signed in.
            && (!requirement.on_demand || matches!(credential, Some(CredentialRef::Oauth)))
            && !self.signed_in(&config.id, requirement).await
        {
            return Err(format!(
                "the source `{0}` is not signed in; run `memcastle source auth {0}`",
                miner.source
            ));
        }
        Ok(())
    }

    /// What the installed source `name` declares under `[permissions.oauth]`, if it is installed and declares it.
    pub(super) async fn oauth_requirement(
        &self,
        name: &str,
    ) -> Option<crate::domain::OAuthRequirement> {
        self.store
            .get_source_package(name)
            .await
            .ok()
            .flatten()
            .and_then(|record| record.manifest.permissions.oauth)
    }

    /// Whether `source` has a credential it could use, asked off the async runtime because the store may be a system
    /// service.
    pub(super) async fn signed_in(
        &self,
        source: &str,
        requirement: crate::domain::OAuthRequirement,
    ) -> bool {
        use crate::domain::AccessTokens as _;
        let credentials = self.credentials.clone();
        let source = source.to_string();
        tokio::task::spawn_blocking(move || credentials.is_signed_in(&source, &requirement))
            .await
            .unwrap_or(false)
    }

    /// A miner for display: its state, and what has been mined from it.
    async fn miner_view(
        &self,
        miner: &MinerDefinition,
        sources: &[SourceRecord],
    ) -> Result<MinerView> {
        let (state, reason) = if !miner.enabled {
            (MinerState::Disabled, None)
        } else {
            match self.check_activation(miner).await {
                Ok(()) => (MinerState::Ready, None),
                Err(reason) => (MinerState::Unavailable, Some(reason)),
            }
        };
        // The record the adapter keeps the cursor on: same source and the miner's locator (a directory's is
        // canonicalised when it is mined, so a symlinked locator still finds it).
        let record = miner
            .locator
            .as_deref()
            .filter(|_| {
                miner.mapping.is_none()
                    && miner.wing.is_none()
                    && miner.room.is_none()
                    && miner
                        .source_config
                        .as_deref()
                        .is_none_or(|id| id == miner.source)
            })
            .and_then(|locator| {
                let canonical = (miner.source == "directory")
                    .then(|| std::fs::canonicalize(locator).ok())
                    .flatten()
                    .map(|p| p.to_string_lossy().into_owned());
                sources.iter().find(|s| {
                    s.source == miner.source
                        && (s.locator == locator
                            || canonical.as_deref() == Some(s.locator.as_str()))
                })
            });
        let documents = match record {
            Some(record) => self.store.count_source_documents(record.id).await?,
            None => 0,
        };
        let config = self
            .source_config(
                miner.source_config.as_deref().unwrap_or(&miner.source),
                Some(&miner.source),
            )
            .await
            .ok();
        let requirement = self.oauth_requirement(&miner.source).await;
        let effective = miner.credential.as_ref().or_else(|| {
            config
                .as_ref()
                .and_then(|config| config.credential.as_ref())
        });
        let credential = match effective {
            Some(c) => Some(CredentialView {
                kind: match c {
                    CredentialRef::Env { .. } => "env",
                    CredentialRef::File { .. } => "file",
                    CredentialRef::Oauth => "oauth",
                }
                .to_string(),
                // For `oauth`, "resolves" means signed in: the same question the activation check asks.
                available: match (c, requirement.clone()) {
                    (CredentialRef::Oauth, Some(requirement)) => {
                        self.signed_in(
                            config
                                .as_ref()
                                .map_or(miner.source.as_str(), |config| config.id.as_str()),
                            requirement,
                        )
                        .await
                    }
                    (CredentialRef::Oauth, None) => false,
                    (other, _) => credential_available(other),
                },
            }),
            None => match requirement {
                Some(requirement) => Some(CredentialView {
                    kind: "oauth".into(),
                    available: self
                        .signed_in(
                            config
                                .as_ref()
                                .map_or(miner.source.as_str(), |config| config.id.as_str()),
                            requirement,
                        )
                        .await,
                }),
                None => None,
            },
        };
        Ok(MinerView {
            name: miner.name.clone(),
            source: miner.source.clone(),
            enabled: miner.enabled,
            state,
            reason,
            locator: miner.locator.clone(),
            wing: miner.wing.clone(),
            room: miner.room.clone(),
            mapping: miner.mapping.clone(),
            source_config: miner.source_config.clone(),
            max_documents: miner.max_documents,
            embedding_profile: miner.embedding_profile.clone(),
            schedule: miner.schedule.clone(),
            credential,
            options: miner.options.clone(),
            config_file: None,
            source_id: record.map(|r| r.id),
            last_run_at: record.and_then(|r| r.last_run_at),
            documents,
        })
    }
}

fn invalid(name: &str, reason: impl Into<String>) -> Error {
    Error::MinerInvalid {
        name: name.to_string(),
        reason: reason.into(),
    }
}

pub(super) fn find<'a>(miners: &'a [MinerDefinition], name: &str) -> Result<&'a MinerDefinition> {
    miners
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| Error::MinerNotFound {
            name: name.to_string(),
        })
}

/// Whether a credential reference points at something that exists. The value itself is never read here.
pub(super) fn credential_available(credential: &CredentialRef) -> bool {
    match credential {
        CredentialRef::Env { name } => std::env::var_os(name).is_some_and(|v| !v.is_empty()),
        CredentialRef::File { path } => Path::new(path).is_file(),
        // Whether a sign-in exists is the daemon's to answer, not a file or a variable to look at.
        CredentialRef::Oauth => false,
    }
}

/// The definition `patch` turns `existing` into, or a new one; checked for shape.
fn apply(
    name: &str,
    existing: Option<&MinerDefinition>,
    patch: MinerPatch,
) -> Result<MinerDefinition> {
    let mut miner = match existing {
        Some(existing) => existing.clone(),
        None => {
            let Some(source) = patch.source.clone() else {
                return Err(invalid(
                    name,
                    format!(
                        "there is no miner `{name}` to change, and a new one needs a `source`; \
                         `memcastle miner set {name} --source <source>` creates it"
                    ),
                ));
            };
            MinerDefinition {
                name: name.to_string(),
                source,
                source_config: None,
                enabled: true,
                locator: None,
                wing: None,
                room: None,
                mapping: None,
                max_documents: None,
                embedding_profile: None,
                schedule: None,
                credential: None,
                options: Map::new(),
            }
        }
    };
    for field in &patch.unset {
        match field.as_str() {
            "locator" => miner.locator = None,
            "wing" => miner.wing = None,
            "credential" => miner.credential = None,
            "room" => miner.room = None,
            "mapping" => miner.mapping = None,
            "source_config" => miner.source_config = None,
            "max_documents" => miner.max_documents = None,
            "embedding_profile" => miner.embedding_profile = None,
            "schedule" => miner.schedule = None,
            other => {
                return Err(invalid(
                    name,
                    format!(
                        "cannot unset `{other}`; the fields that can be cleared are locator, wing, credential"
                    ),
                ));
            }
        }
    }
    if let Some(source) = patch.source {
        miner.source = source;
    }
    if patch.source_config.is_some() {
        miner.source_config = patch.source_config;
    }
    if let Some(enabled) = patch.enabled {
        miner.enabled = enabled;
    }
    if patch.locator.is_some() {
        miner.locator = patch.locator;
    }
    if patch.wing.is_some() {
        miner.wing = patch.wing;
    }
    if patch.room.is_some() {
        miner.room = patch.room;
    }
    if patch.mapping.is_some() {
        miner.mapping = patch.mapping;
    }
    if patch.max_documents.is_some() {
        miner.max_documents = patch.max_documents;
    }
    if patch.embedding_profile.is_some() {
        miner.embedding_profile = patch.embedding_profile;
    }
    if patch.schedule.is_some() {
        miner.schedule = patch.schedule;
    }
    if patch.credential.is_some() {
        miner.credential = patch.credential;
    }
    for key in &patch.unset_options {
        miner.options.remove(key);
    }
    miner.options.extend(patch.options);
    miner.validate().map_err(|reason| invalid(name, reason))?;
    Ok(miner)
}
