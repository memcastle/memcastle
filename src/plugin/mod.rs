//! Plugin authoring contracts and archive validation, independent of the daemon.

pub mod manifest;
pub mod package;
pub mod signing;

/// Immutable descriptor at a plugin repository's root (not included in the archive).
pub const MANIFEST_FILE: &str = "plugin.toml";
/// Complete release metadata, generated beside the descriptor and embedded in the archive.
pub const RELEASE_FILE: &str = "manifest.toml";
