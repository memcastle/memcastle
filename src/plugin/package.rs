//! Deterministic multi-module archives with bounded, manifest-checked extraction.

use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::domain::{PluginManifest, PluginModuleKind, sha256_hex};
use crate::error::{Error, Result};

use super::{MANIFEST_FILE, RELEASE_FILE, manifest};

/// Maximum compressed plugin upload/download size.
pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
/// Maximum number of files in a plugin archive, including unexpected entries.
pub const MAX_ARCHIVE_FILES: usize = 1024;
/// Maximum expanded plugin size; total includes files the manifest does not name.
pub const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

/// Validated contents of a plugin release.
#[derive(Debug, Clone)]
pub struct PluginPackage {
    /// Parsed parent contract.
    pub manifest: PluginManifest,
    /// Original manifest bytes, retained for reproducible installation.
    pub manifest_text: String,
    /// Every regular archive file under its checked path.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// The archive a publisher just built from reviewed, prebuilt artifacts.
#[derive(Debug, Serialize)]
pub struct PackagedPlugin {
    /// Destination of the deterministic release archive.
    pub archive: PathBuf,
    /// External GNU-style checksum for the archive, placed beside it.
    pub checksum: PathBuf,
    /// SHA-256 of the exact archive bytes.
    pub sha256: String,
}

/// Inspect a project's typed module inventory before building its artifacts.
///
/// # Errors
///
/// Rejects a missing or malformed root manifest.
pub fn inspect_manifest(path: &Path) -> Result<PluginManifest> {
    if path.file_name() != Some(std::ffi::OsStr::new(MANIFEST_FILE)) {
        return Err(invalid("the root manifest must be named plugin.toml"));
    }
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let bytes = read_project_file(root, MANIFEST_FILE)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("plugin.toml is not UTF-8"))?;
    let mut descriptor = manifest::parse_descriptor(text)?;
    let release = if root.join(RELEASE_FILE).is_file() {
        let bytes = read_project_file(root, RELEASE_FILE)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| invalid("manifest.toml is not UTF-8"))?;
        manifest::parse_descriptor(text)?
    } else {
        descriptor.clone()
    };
    if release.plugin.version.is_empty() {
        return Err(invalid("manifest.toml must declare a release version"));
    }
    descriptor.plugin.version = release.plugin.version;
    for module in &mut descriptor.modules {
        if let Some(released) = release
            .modules
            .iter()
            .find(|released| released.kind == module.kind && released.id == module.id)
        {
            module.version = released.version.clone();
        }
    }
    Ok(descriptor)
}

/// Package a manifest's already-built modules without changing the manifest.
///
/// # Errors
///
/// Rejects unsafe paths and missing/invalid artifacts.
pub fn package_manifest(path: &Path, output: Option<&Path>) -> Result<PackagedPlugin> {
    if path.file_name() != Some(std::ffi::OsStr::new(MANIFEST_FILE)) {
        return Err(invalid("the root manifest must be named plugin.toml"));
    }
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let plugin = inspect_manifest(path)?;
    let bytes = pack_project(root)?;
    let release = unpack(&bytes)?;
    std::fs::write(root.join(RELEASE_FILE), &release.files[RELEASE_FILE])
        .map_err(|e| Error::io(root.join(RELEASE_FILE).display().to_string(), e))?;
    let destination = output.map_or_else(
        || {
            root.join("dist").join(format!(
                "{}-{}.tar.gz",
                plugin.plugin.id, plugin.plugin.version
            ))
        },
        Path::to_path_buf,
    );
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent.display().to_string(), e))?;
    }
    std::fs::write(&destination, &bytes)
        .map_err(|e| Error::io(destination.display().to_string(), e))?;
    let checksum = destination.with_file_name(format!(
        "{}.sha256sum",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim_end_matches(".tar.gz")
    ));
    let archive_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("archive filename must be UTF-8"))?;
    let digest = sha256_hex(&bytes);
    std::fs::write(&checksum, format!("{digest}  {archive_name}\n"))
        .map_err(|e| Error::io(checksum.display().to_string(), e))?;
    Ok(PackagedPlugin {
        archive: destination,
        checksum,
        sha256: digest,
    })
}

fn invalid(message: impl Into<String>) -> Error {
    Error::PluginPackageInvalid {
        message: message.into(),
    }
}

/// Pack validated plugin files in lexicographic order with fixed metadata.
///
/// # Errors
///
/// [`Error::PluginPackageInvalid`] if the input is incomplete or the archive cannot be written.
pub fn pack(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut all = files.clone();
    let descriptor_bytes = all
        .remove(MANIFEST_FILE)
        .ok_or_else(|| invalid("missing plugin.toml descriptor"))?;
    let mut descriptor = manifest::parse_descriptor(
        std::str::from_utf8(&descriptor_bytes).map_err(|_| invalid("plugin.toml is not UTF-8"))?,
    )?;
    let seed = if let Some(bytes) = all.remove(RELEASE_FILE) {
        manifest::parse_descriptor(
            std::str::from_utf8(&bytes).map_err(|_| invalid("manifest.toml is not UTF-8"))?,
        )?
    } else {
        descriptor.clone()
    };
    if seed.plugin.version.is_empty() {
        return Err(invalid("manifest.toml must declare a release version"));
    }
    descriptor.plugin.version = seed.plugin.version;
    for module in &mut descriptor.modules {
        module.version = match module.kind {
            PluginModuleKind::Source => {
                let text = std::str::from_utf8(
                    all.get(&module.manifest)
                        .ok_or_else(|| invalid(format!("missing `{}`", module.manifest)))?,
                )
                .map_err(|_| invalid(format!("module `{}` manifest is not UTF-8", module.id)))?;
                crate::source::manifest::parse(text, &crate::mining::registry::BUILTIN_NAMES)?
                    .source
                    .version
            }
            PluginModuleKind::Integration => {
                let text = std::str::from_utf8(
                    all.get(&module.manifest)
                        .ok_or_else(|| invalid(format!("missing `{}`", module.manifest)))?,
                )
                .map_err(|_| invalid(format!("module `{}` manifest is not UTF-8", module.id)))?;
                crate::integration::manifest::parse(text)?
                    .integration
                    .version
            }
            PluginModuleKind::EmbeddingProvider => seed
                .modules
                .iter()
                .find(|entry| entry.kind == module.kind && entry.id == module.id)
                .map_or_else(|| module.version.clone(), |entry| entry.version.clone()),
        };
        module.manifest_sha256 = sha256_hex(
            all.get(&module.manifest)
                .ok_or_else(|| invalid(format!("missing `{}`", module.manifest)))?,
        );
        module.sha256 = sha256_hex(
            all.get(&module.entry)
                .ok_or_else(|| invalid(format!("missing `{}`", module.entry)))?,
        );
    }
    let mut checksums = BTreeMap::new();
    for (path, bytes) in &all {
        if path.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(invalid(format!(
                "archive path `{path}` contains a control character"
            )));
        }
        checksums.insert(path.clone(), sha256_hex(bytes));
    }
    descriptor.checksums = checksums;
    all.insert(
        RELEASE_FILE.to_string(),
        toml::to_string(&descriptor)
            .map_err(|e| invalid(format!("cannot format manifest.toml: {e}")))?
            .into_bytes(),
    );
    validate_files(&all)?;
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tarball = tar::Builder::new(encoder);
    for (path, bytes) in &all {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_entry_type(tar::EntryType::Regular);
        tarball
            .append_data(&mut header, path, bytes.as_slice())
            .map_err(|e| invalid(format!("cannot archive `{path}`: {e}")))?;
    }
    let archive = tarball
        .into_inner()
        .and_then(flate2::write::GzEncoder::finish)
        .map_err(|e| invalid(format!("cannot finish the archive: {e}")))?;
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err(invalid("the compressed archive exceeds 64 MiB"));
    }
    Ok(archive)
}

/// Build an archive from a local plugin project, reading only declared module artifacts.
///
/// # Errors
///
/// Refuses symlinks, missing artifacts and oversized projects; ignores build outputs not declared by the manifest.
pub fn pack_project(root: &Path) -> Result<Vec<u8>> {
    let manifest = read_project_file(root, MANIFEST_FILE)?;
    pack_project_with_manifest(root, &manifest)
}

fn pack_project_with_manifest(root: &Path, manifest: &[u8]) -> Result<Vec<u8>> {
    let mut files = BTreeMap::new();
    let text = std::str::from_utf8(manifest).map_err(|_| invalid("plugin.toml is not UTF-8"))?;
    let plugin = manifest::parse_descriptor(text)?;
    files.insert(MANIFEST_FILE.to_string(), manifest.to_vec());
    if root.join(RELEASE_FILE).is_file() {
        files.insert(
            RELEASE_FILE.to_string(),
            read_project_file(root, RELEASE_FILE)?,
        );
    }
    for module in &plugin.modules {
        files.insert(
            module.manifest.clone(),
            read_project_file(root, &module.manifest)?,
        );
        files.insert(
            module.entry.clone(),
            read_project_file(root, &module.entry)?,
        );
        if module.kind == PluginModuleKind::Integration {
            let text = std::str::from_utf8(&files[&module.manifest]).map_err(|_| {
                invalid(format!("integration `{}` manifest is not UTF-8", module.id))
            })?;
            let integration = crate::integration::manifest::parse(text)?;
            let prefix = module.manifest.rsplit_once('/').map_or("", |(dir, _)| dir);
            for asset in &integration.assets {
                let path = format!("{prefix}/{}", asset.from);
                collect_project_files(root, &path, &mut files)?;
            }
            for skill in &integration.skills {
                let path = if skill.local {
                    format!("{prefix}/skills/{}", skill.name)
                } else {
                    format!("skills/{}", skill.name)
                };
                collect_project_files(root, &path, &mut files)?;
            }
        }
    }
    pack(&files)
}

fn checked_project_path(root: &Path, name: &str) -> Result<PathBuf> {
    if !manifest::safe_path(name) || name.split('/').count() > 16 {
        return Err(invalid(format!("unsafe project file `{name}`")));
    }
    let mut path = root.to_path_buf();
    for part in Path::new(name).components() {
        path.push(part);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|e| invalid(format!("cannot read `{}`: {e}", path.display())))?;
        if metadata.file_type().is_symlink() {
            return Err(invalid(format!(
                "project file `{name}` traverses a symbolic link"
            )));
        }
    }
    Ok(path)
}

fn read_project_file(root: &Path, name: &str) -> Result<Vec<u8>> {
    let path = checked_project_path(root, name)?;
    let metadata =
        std::fs::metadata(&path).map_err(|e| invalid(format!("cannot read `{name}`: {e}")))?;
    if !metadata.is_file() || metadata.len() > MAX_UNPACKED_BYTES {
        return Err(invalid(format!(
            "`{name}` is not a regular file within the size limit"
        )));
    }
    std::fs::read(path).map_err(|e| invalid(format!("cannot read `{name}`: {e}")))
}

fn collect_project_files(
    root: &Path,
    name: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if files.len() > 1024 {
        return Err(invalid("plugin project has more than 1024 files"));
    }
    let path = checked_project_path(root, name)?;
    if path.is_dir() {
        let entries =
            std::fs::read_dir(&path).map_err(|e| invalid(format!("cannot list `{name}`: {e}")))?;
        for entry in entries {
            let entry = entry.map_err(|e| invalid(format!("cannot list `{name}`: {e}")))?;
            let child = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid(format!("non-UTF-8 file under `{name}`")))?;
            collect_project_files(root, &format!("{name}/{child}"), files)?;
        }
    } else {
        files.insert(name.to_string(), read_project_file(root, name)?);
    }
    Ok(())
}

/// Read a release, rejecting links, duplicate paths and oversized or unlisted files.
///
/// # Errors
///
/// [`Error::PluginPackageInvalid`] for an unsafe/incomplete archive, or a manifest error.
pub fn unpack(archive: &[u8]) -> Result<PluginPackage> {
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err(invalid("the compressed archive exceeds 64 MiB"));
    }
    let reader = flate2::read::GzDecoder::new(archive);
    let mut tarball = tar::Archive::new(reader);
    let mut files = BTreeMap::new();
    let mut total = 0_u64;
    for entry in tarball
        .entries()
        .map_err(|e| invalid(format!("not a gzip tar archive: {e}")))?
    {
        let mut entry = entry.map_err(|e| invalid(format!("damaged tar entry: {e}")))?;
        if !entry.header().entry_type().is_file() {
            return Err(invalid(
                "only regular files are accepted; links and special files are refused",
            ));
        }
        let path = entry
            .path()
            .map_err(|e| invalid(format!("unreadable archive path: {e}")))?
            .into_owned();
        let path = path
            .to_str()
            .ok_or_else(|| invalid("archive paths must be UTF-8"))?;
        if !manifest::safe_path(path) {
            return Err(invalid(format!("unsafe archive path `{path}`")));
        }
        // Reject a flood of empty entries before allocating their paths or contents.
        if files.len() >= MAX_ARCHIVE_FILES {
            return Err(invalid("plugin archive has more than 1024 files"));
        }
        total = total.saturating_add(entry.size());
        if total > MAX_UNPACKED_BYTES {
            return Err(invalid("the expanded archive exceeds 256 MiB"));
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| invalid(format!("cannot read `{path}`: {e}")))?;
        if files.insert(path.to_string(), bytes).is_some() {
            return Err(invalid(format!("duplicate archive path `{path}`")));
        }
    }
    validate_files(&files)
}

fn validate_files(files: &BTreeMap<String, Vec<u8>>) -> Result<PluginPackage> {
    if files.len() > MAX_ARCHIVE_FILES {
        return Err(invalid("plugin archive has more than 1024 files"));
    }
    let total: u64 = files.values().map(|bytes| bytes.len() as u64).sum();
    if total > MAX_UNPACKED_BYTES {
        return Err(invalid("the expanded archive exceeds 256 MiB"));
    }
    for path in files.keys() {
        if !manifest::safe_path(path) {
            return Err(invalid(format!("unsafe archive path `{path}`")));
        }
    }
    let text = std::str::from_utf8(
        files
            .get(RELEASE_FILE)
            .ok_or_else(|| invalid("missing manifest.toml"))?,
    )
    .map_err(|_| invalid("manifest.toml is not UTF-8"))?;
    let root = manifest::parse(text)?;
    verify_checksums(files, &root.checksums)?;
    let mut allowed = HashSet::from([RELEASE_FILE]);
    for module in &root.modules {
        for (path, expected) in [
            (&module.manifest, &module.manifest_sha256),
            (&module.entry, &module.sha256),
        ] {
            let bytes = files
                .get(path)
                .ok_or_else(|| invalid(format!("module `{}` is missing `{path}`", module.id)))?;
            if sha256_hex(bytes) != *expected {
                return Err(invalid(format!(
                    "module `{}`: `{path}` does not match its manifest.toml digest",
                    module.id
                )));
            }
            allowed.insert(path.as_str());
        }
        let text = std::str::from_utf8(&files[&module.manifest])
            .map_err(|_| invalid(format!("module `{}` manifest is not UTF-8", module.id)))?;
        match module.kind {
            PluginModuleKind::Source => {
                let source =
                    crate::source::manifest::parse(text, &crate::mining::registry::BUILTIN_NAMES)?;
                if source.source.name != module.id || source.source.version != module.version {
                    return Err(invalid(format!(
                        "source module `{}` identity/version differs from its own manifest",
                        module.id
                    )));
                }
                if module.contract.as_deref() != Some(source.compatibility.contract.as_str()) {
                    return Err(invalid(format!(
                        "source module `{}` contract differs from its source manifest",
                        module.id
                    )));
                }
                if !files[&module.entry].starts_with(b"\0asm") {
                    return Err(invalid(format!(
                        "source module `{}` is not WebAssembly",
                        module.id
                    )));
                }
            }
            PluginModuleKind::Integration => {
                let integration = crate::integration::manifest::parse(text)?;
                if integration.integration.id != module.id
                    || integration.integration.version != module.version
                {
                    return Err(invalid(format!(
                        "integration module `{}` identity/version differs from its own manifest",
                        module.id
                    )));
                }
                if module.contract.is_some() {
                    return Err(invalid(format!(
                        "integration module `{}` has no WASM contract",
                        module.id
                    )));
                }
                let prefix = module.manifest.rsplit_once('/').map_or("", |(dir, _)| dir);
                let under = |path: &str| format!("{prefix}/{path}");
                for asset in &integration.assets {
                    let path = under(&asset.from);
                    let members: Vec<_> = files
                        .keys()
                        .filter(|name| **name == path || name.starts_with(&format!("{path}/")))
                        .collect();
                    if members.is_empty() {
                        return Err(invalid(format!(
                            "integration module `{}` is missing asset `{}`",
                            module.id, asset.from
                        )));
                    }
                    allowed.extend(members.into_iter().map(String::as_str));
                }
                for skill in &integration.skills {
                    let path = if skill.local {
                        under(&format!("skills/{}", skill.name))
                    } else {
                        format!("skills/{}", skill.name)
                    };
                    let members: Vec<_> = files
                        .keys()
                        .filter(|name| name.starts_with(&format!("{path}/")))
                        .collect();
                    if members.is_empty() {
                        return Err(invalid(format!(
                            "integration module `{}` is missing skill `{}`",
                            module.id, skill.name
                        )));
                    }
                    allowed.extend(members.into_iter().map(String::as_str));
                }
            }
            PluginModuleKind::EmbeddingProvider => {
                // The metadata can be discovered, but #256 defines how its code is run.
            }
        }
    }
    if let Some(path) = files.keys().find(|path| !allowed.contains(path.as_str())) {
        return Err(invalid(format!("unlisted plugin file `{path}`")));
    }
    Ok(PluginPackage {
        manifest: root,
        manifest_text: text.to_string(),
        files: files.clone(),
    })
}

fn verify_checksums(
    files: &BTreeMap<String, Vec<u8>>,
    checksums: &BTreeMap<String, String>,
) -> Result<()> {
    for (path, expected) in checksums {
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || path == RELEASE_FILE
            || !manifest::safe_path(path)
            || path.bytes().any(|b| b.is_ascii_control())
        {
            return Err(invalid(format!(
                "invalid manifest.toml checksum entry `{path}`"
            )));
        }
        let actual = files
            .get(path)
            .ok_or_else(|| invalid(format!("manifest.toml names missing file `{path}`")))?;
        if sha256_hex(actual) != *expected {
            return Err(invalid(format!(
                "manifest.toml checksum does not match `{path}`"
            )));
        }
    }
    if checksums.len() != files.len() - 1 {
        return Err(invalid(
            "manifest.toml must checksum every other archive file exactly once",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PluginInfo, PluginModule, PluginModuleKind};

    fn fixture() -> BTreeMap<String, Vec<u8>> {
        let source = b"format = 1\n[source]\nname = 'one'\nversion = '0.1.0'\ndescription = 'one'\n[compatibility]\ncontract = '0.4.0'\nmemcastle = '>=0.4.0'\n";
        let second = b"format = 1\n[source]\nname = 'two'\nversion = '0.1.0'\ndescription = 'two'\n[compatibility]\ncontract = '0.4.0'\nmemcastle = '>=0.4.0'\n";
        let component = b"\0asm\x01\0\0\0";
        let modules = [("one", source.as_slice()), ("two", second.as_slice())]
            .into_iter()
            .map(|(id, bytes)| PluginModule {
                id: id.to_string(),
                kind: PluginModuleKind::Source,
                version: "0.1.0".to_string(),
                manifest: format!("modules/{id}/memcastle-source.toml"),
                manifest_sha256: sha256_hex(bytes),
                entry: format!("modules/{id}/source.wasm"),
                sha256: sha256_hex(component),
                optional: false,
                contract: Some("0.4.0".to_string()),
                config_schema: None,
            })
            .collect::<Vec<_>>();
        let manifest = PluginManifest {
            format: 1,
            plugin: PluginInfo {
                id: "example".to_string(),
                version: "0.1.0".to_string(),
                provider: "example".to_string(),
                description: "Two independent sources".to_string(),
                repository: "https://github.com/example/example".to_string(),
                license: "MIT".to_string(),
            },
            memcastle: ">=0.4.0".to_string(),
            dependencies: Vec::new(),
            shared_config_schema: None,
            authentication: None,
            modules,
            checksums: BTreeMap::new(),
        };
        let mut files = BTreeMap::from([(
            MANIFEST_FILE.to_string(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        )]);
        for (id, bytes) in [("one", source.as_slice()), ("two", second.as_slice())] {
            files.insert(
                format!("modules/{id}/memcastle-source.toml"),
                bytes.to_vec(),
            );
            files.insert(format!("modules/{id}/source.wasm"), component.to_vec());
        }
        files
    }

    #[test]
    fn two_sources_are_packaged_together_without_activation_state() {
        let files = fixture();
        let bytes = pack(&files).unwrap();
        assert_eq!(pack(&files).unwrap(), bytes);
        let package = unpack(&bytes).unwrap();
        assert_eq!(package.manifest.modules.len(), 2);
        let mut archived = package.files;
        assert!(archived.remove(RELEASE_FILE).is_some());
        let mut expected = files;
        expected.remove(MANIFEST_FILE);
        assert_eq!(archived, expected);
    }

    #[test]
    fn a_static_descriptor_is_merged_with_reviewed_release_metadata() {
        let mut files = fixture();
        let seed = String::from_utf8(files.remove(MANIFEST_FILE).unwrap()).unwrap();
        let mut descriptor: toml_edit::DocumentMut = seed.parse().unwrap();
        descriptor["plugin"]
            .as_table_mut()
            .unwrap()
            .remove("version");
        for module in descriptor["modules"]
            .as_array_of_tables_mut()
            .unwrap()
            .iter_mut()
        {
            for key in ["version", "sha256", "manifest_sha256"] {
                module.remove(key);
            }
        }
        let unchanged = descriptor.to_string();
        files.insert(MANIFEST_FILE.into(), unchanged.as_bytes().to_vec());
        files.insert(RELEASE_FILE.into(), seed.into_bytes());
        let packed = unpack(&pack(&files).unwrap()).unwrap();
        assert_eq!(packed.manifest.plugin.version, "0.1.0");
        assert_eq!(packed.manifest.modules.len(), 2);
        assert_eq!(packed.manifest.checksums.len(), 4);
        assert!(!packed.files.contains_key(MANIFEST_FILE));
        assert_eq!(
            std::str::from_utf8(&files[MANIFEST_FILE]).unwrap(),
            unchanged
        );
    }

    #[test]
    fn a_publisher_keeps_the_manifest_immutable_and_emits_both_checksum_files() {
        let root = tempfile::tempdir().unwrap();
        let mut files = fixture();
        let text = String::from_utf8(files.remove(MANIFEST_FILE).unwrap()).unwrap();
        let manifest = text.replace(
            &sha256_hex(&files["modules/one/source.wasm"]),
            &"0".repeat(64),
        );
        std::fs::write(root.path().join(MANIFEST_FILE), &manifest).unwrap();
        for (name, bytes) in files {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let first = package_manifest(&root.path().join(MANIFEST_FILE), None).unwrap();
        let archive = std::fs::read(&first.archive).unwrap();
        assert_eq!(first.sha256, sha256_hex(&archive));
        let packaged = unpack(&archive).unwrap();
        assert_eq!(packaged.manifest.modules.len(), 2);
        assert!(!packaged.files.contains_key(MANIFEST_FILE));
        assert_eq!(
            packaged.files[RELEASE_FILE],
            packaged.manifest_text.as_bytes()
        );
        assert_eq!(packaged.manifest.plugin.version, "0.1.0");
        assert_eq!(
            std::fs::read(root.path().join(MANIFEST_FILE)).unwrap(),
            manifest.as_bytes()
        );
        let expected = format!("{}  example-0.1.0.tar.gz\n", first.sha256);
        assert_eq!(std::fs::read_to_string(&first.checksum).unwrap(), expected);
        let again = package_manifest(&root.path().join(MANIFEST_FILE), None).unwrap();
        assert_eq!(again.sha256, first.sha256);
        assert_eq!(
            std::fs::read(root.path().join(MANIFEST_FILE)).unwrap(),
            manifest.as_bytes()
        );
    }

    #[test]
    fn an_artifact_changed_after_the_archive_checksum_is_created_is_refused() {
        let mut files = unpack(&pack(&fixture()).unwrap()).unwrap().files;
        files.insert(
            "modules/two/source.wasm".to_string(),
            b"\0asmreplaced".to_vec(),
        );
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tarball = tar::Builder::new(encoder);
        for (path, bytes) in &files {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            tarball
                .append_data(&mut header, path, bytes.as_slice())
                .unwrap();
        }
        let archive = tarball.into_inner().unwrap().finish().unwrap();
        assert!(matches!(
            unpack(&archive),
            Err(Error::PluginPackageInvalid { .. })
        ));
    }

    #[test]
    fn an_archive_without_the_required_manifest_is_refused() {
        let files = fixture();
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tarball = tar::Builder::new(encoder);
        for (path, bytes) in &files {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            tarball
                .append_data(&mut header, path, bytes.as_slice())
                .unwrap();
        }
        let archive = tarball.into_inner().unwrap().finish().unwrap();
        let error = unpack(&archive).unwrap_err().to_string();
        assert!(error.contains("manifest.toml"), "{error}");
    }

    #[test]
    fn an_extra_file_or_path_escape_is_refused() {
        let mut files = fixture();
        files.insert("../outside".to_string(), Vec::new());
        assert!(matches!(
            pack(&files),
            Err(Error::PluginPackageInvalid { .. })
        ));
        files.remove("../outside");
        files.insert("modules/unreviewed/source.wasm".to_string(), Vec::new());
        assert!(matches!(
            pack(&files),
            Err(Error::PluginPackageInvalid { .. })
        ));
        files.remove("modules/unreviewed/source.wasm");
        files.insert("modules/one/./source.wasm".to_string(), Vec::new());
        assert!(matches!(
            pack(&files),
            Err(Error::PluginPackageInvalid { .. })
        ));
    }

    #[test]
    fn an_archive_with_too_many_empty_entries_is_refused_before_extraction() {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tarball = tar::Builder::new(encoder);
        for index in 0..=MAX_ARCHIVE_FILES {
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            tarball
                .append_data(&mut header, format!("empty-{index}"), &[][..])
                .unwrap();
        }
        let archive = tarball.into_inner().unwrap().finish().unwrap();
        let error = unpack(&archive).unwrap_err().to_string();
        assert!(error.contains("more than 1024 files"), "{error}");
    }

    #[test]
    fn a_module_cannot_point_at_another_modules_artifact_directory() {
        let files = fixture();
        let mut manifest =
            manifest::parse(std::str::from_utf8(&files[MANIFEST_FILE]).unwrap()).unwrap();
        manifest.modules[0].entry = "modules/two/renamed-source.wasm".into();
        assert!(matches!(
            manifest::validate(&manifest),
            Err(Error::PluginManifestInvalid { .. })
        ));
    }

    #[test]
    fn an_archive_link_is_refused_before_following_any_path() {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tarball = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("../../outside").unwrap();
        header.set_size(0);
        header.set_cksum();
        tarball
            .append_data(&mut header, "modules/evil/source.wasm", &[][..])
            .unwrap();
        let archive = tarball.into_inner().unwrap().finish().unwrap();
        assert!(matches!(
            unpack(&archive),
            Err(Error::PluginPackageInvalid { .. })
        ));
    }

    #[test]
    fn an_empty_plugin_has_no_implicitly_activated_modules() {
        let mut files = fixture();
        let mut manifest =
            manifest::parse(std::str::from_utf8(&files[MANIFEST_FILE]).unwrap()).unwrap();
        manifest.modules.clear();
        files.retain(|name, _| name == MANIFEST_FILE);
        files.insert(
            MANIFEST_FILE.to_string(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        );
        assert!(
            unpack(&pack(&files).unwrap())
                .unwrap()
                .manifest
                .modules
                .is_empty()
        );
    }

    #[test]
    fn a_source_and_integration_with_the_same_id_remain_distinct_in_the_archive() {
        let mut files = fixture();
        let mut manifest =
            manifest::parse(std::str::from_utf8(&files[MANIFEST_FILE]).unwrap()).unwrap();
        manifest.modules.retain(|module| module.id == "one");
        files.remove("modules/two/memcastle-source.toml");
        files.remove("modules/two/source.wasm");
        let integration = b"format = 1\n[integration]\nid = 'one'\nversion = '0.1.0'\ndescription = 'An assistant integration'\n[compatibility]\nmemcastle = '>=0.4.0'\n[agent]\nkind = 'opencode'\nentry = 'dist/plugin.js'\n[[assets]]\nfrom = 'dist'\nto = '.'\n";
        let entry = b"export default () => {}";
        manifest.modules.push(PluginModule {
            id: "one".to_string(),
            kind: PluginModuleKind::Integration,
            version: "0.1.0".to_string(),
            manifest: "modules/one/memcastle-integration.toml".to_string(),
            manifest_sha256: sha256_hex(integration),
            entry: "modules/one/dist/plugin.js".to_string(),
            sha256: sha256_hex(entry),
            optional: false,
            contract: None,
            config_schema: None,
        });
        files.insert(
            "modules/one/memcastle-integration.toml".to_string(),
            integration.to_vec(),
        );
        files.insert("modules/one/dist/plugin.js".to_string(), entry.to_vec());
        // The installer needs the entire declared asset tree, including chunks beside the entry point.
        files.insert(
            "modules/one/dist/chunks/runtime.js".to_string(),
            b"export const chunk = true;".to_vec(),
        );
        files.insert(
            MANIFEST_FILE.to_string(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        );
        let package = unpack(&pack(&files).unwrap()).unwrap();
        assert_eq!(package.manifest.modules[0].kind, PluginModuleKind::Source);
        assert_eq!(
            package.manifest.modules[1].kind,
            PluginModuleKind::Integration
        );
        let root = tempfile::tempdir().unwrap();
        for (path, bytes) in &files {
            let target = root.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
        let mut archived = unpack(&pack_project(root.path()).unwrap()).unwrap().files;
        assert!(archived.remove(RELEASE_FILE).is_some());
        let mut expected = files;
        expected.remove(MANIFEST_FILE);
        assert_eq!(
            archived, expected,
            "packaging from a provider checkout must retain the integration's nested assets"
        );
    }

    #[test]
    fn an_embedding_module_is_discoverable_without_pretending_to_be_a_source_adapter() {
        let mut files = fixture();
        let mut manifest =
            manifest::parse(std::str::from_utf8(&files[MANIFEST_FILE]).unwrap()).unwrap();
        let entry = b"future embedding contract";
        let config = b"{\"format\":1}";
        manifest.modules.push(PluginModule {
            id: "embeddings".into(),
            kind: PluginModuleKind::EmbeddingProvider,
            version: "0.1.0".into(),
            manifest: "modules/embeddings/manifest.json".into(),
            manifest_sha256: sha256_hex(config),
            entry: "modules/embeddings/provider.bin".into(),
            sha256: sha256_hex(entry),
            optional: true,
            contract: None,
            config_schema: None,
        });
        files.insert("modules/embeddings/manifest.json".into(), config.to_vec());
        files.insert("modules/embeddings/provider.bin".into(), entry.to_vec());
        files.insert(
            MANIFEST_FILE.into(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        );
        let inspected = unpack(&pack(&files).unwrap()).unwrap();
        assert_eq!(
            inspected.manifest.modules[2].kind,
            PluginModuleKind::EmbeddingProvider
        );
    }

    #[test]
    fn a_local_plugin_project_packages_only_its_declared_artifacts() {
        let root = tempfile::tempdir().unwrap();
        for (path, bytes) in fixture() {
            let target = root.path().join(&path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
        std::fs::write(root.path().join("Cargo.toml"), b"build-only metadata").unwrap();
        let packed = pack_project(root.path()).unwrap();
        assert!(!unpack(&packed).unwrap().files.contains_key("Cargo.toml"));
    }
}
