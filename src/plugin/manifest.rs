//! Parse and validate the parent manifest before touching its artifacts.

use std::collections::HashSet;
use std::path::{Component, Path};

use crate::domain::{PluginManifest, PluginModuleKind, is_valid_source_name};
use crate::error::{Error, Result};

fn invalid(message: impl Into<String>) -> Error {
    Error::PluginManifestInvalid {
        message: message.into(),
    }
}

/// Reject traversal and ambiguous spellings even before an archive is read.
#[must_use]
pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        // On Windows a colon names an alternate data stream even below a seemingly safe module directory.
        && !path.contains(':')
        // `Path::components` normalizes `.` and doubled separators; different archive names must never alias one file.
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Read the strictly versioned plugin contract.
///
/// # Errors
///
/// [`Error::PluginManifestInvalid`] identifies the first malformed field.
pub fn parse(text: &str) -> Result<PluginManifest> {
    let manifest: PluginManifest =
        toml::from_str(text).map_err(|e| invalid(format!("manifest.toml: {}", e.message())))?;
    validate(&manifest)?;
    Ok(manifest)
}

/// Validate the immutable descriptor before release metadata is applied.
///
/// # Errors
///
/// Rejects invalid identities, paths and contracts without requiring release versions or digests.
pub fn parse_descriptor(text: &str) -> Result<PluginManifest> {
    let descriptor: PluginManifest =
        toml::from_str(text).map_err(|e| invalid(format!("plugin.toml: {}", e.message())))?;
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

/// Check descriptor fields independently of the generated manifest.toml.
///
/// # Errors
///
/// Reports malformed identities, version strings, paths or constraints.
pub fn validate_descriptor(manifest: &PluginManifest) -> Result<()> {
    validate_fields(manifest, false)
}

/// Check identities, version requirements, paths and artifact digests.
///
/// # Errors
///
/// [`Error::PluginManifestInvalid`] identifies the first malformed field.
pub fn validate(manifest: &PluginManifest) -> Result<()> {
    validate_fields(manifest, true)
}

fn validate_fields(manifest: &PluginManifest, require_release_fields: bool) -> Result<()> {
    if manifest.format != crate::domain::plugin::PLUGIN_FORMAT {
        return Err(invalid(format!(
            "format {} is unsupported; upgrade MemCastle or use format {}",
            manifest.format,
            crate::domain::plugin::PLUGIN_FORMAT
        )));
    }
    let info = &manifest.plugin;
    if manifest.modules.len() > 64 || manifest.dependencies.len() > 64 {
        return Err(invalid(
            "a plugin may declare at most 64 modules and 64 dependencies",
        ));
    }
    for (field, value) in [("plugin.id", &info.id), ("plugin.provider", &info.provider)] {
        if !is_valid_source_name(value) {
            return Err(invalid(format!(
                "{field} `{value}` must be a lowercase hyphenated identifier"
            )));
        }
    }
    if require_release_fields || !info.version.is_empty() {
        version(&info.version, "plugin.version")?;
    }
    requirement(&manifest.memcastle, "memcastle")?;
    if manifest
        .shared_config_schema
        .as_ref()
        .is_some_and(|schema| !schema.is_object())
    {
        return Err(invalid("shared_config_schema must be a JSON Schema object"));
    }
    if let Some(authentication) = &manifest.authentication
        && (!matches!(
            authentication.kind.as_str(),
            "oauth_device" | "oauth_browser"
        ) || !crate::domain::is_valid_source_name(&authentication.credential))
    {
        return Err(invalid(
            "authentication must name a supported public scheme and a credential reference, never a token",
        ));
    }
    if info.description.trim().is_empty() || info.description.contains('\n') {
        return Err(invalid("plugin.description must be one non-empty line"));
    }
    if !crate::domain::is_secure_endpoint(&info.repository) || info.license.trim().is_empty() {
        return Err(invalid(
            "plugin.repository must be an HTTPS repository URL (or loopback HTTP), and plugin.license is required",
        ));
    }
    let mut dependencies = HashSet::new();
    for dependency in &manifest.dependencies {
        if !is_valid_source_name(&dependency.id)
            || dependency.id == info.id
            || !dependencies.insert(&dependency.id)
        {
            return Err(invalid(format!(
                "dependency `{}` is invalid, duplicated or self-referential",
                dependency.id
            )));
        }
        requirement(&dependency.version, "dependency.version")?;
    }
    let mut modules = HashSet::new();
    let mut paths = HashSet::new();
    for module in &manifest.modules {
        // A provider can expose a source and an agent integration with the same public name.
        if !is_valid_source_name(&module.id) || !modules.insert((module.kind, &module.id)) {
            return Err(invalid(format!(
                "module `{}` is invalid or duplicated",
                module.id
            )));
        }
        if require_release_fields || !module.version.is_empty() {
            version(&module.version, "module.version")?;
        }
        if module
            .config_schema
            .as_ref()
            .is_some_and(|schema| !schema.is_object())
        {
            return Err(invalid(format!(
                "module `{}` config_schema must be a JSON Schema object",
                module.id
            )));
        }
        for path in [&module.manifest, &module.entry] {
            if !safe_path(path) || !paths.insert(path) {
                return Err(invalid(format!(
                    "module `{}` has an unsafe or duplicated artifact path `{path}`",
                    module.id
                )));
            }
            if !path.starts_with(&format!("modules/{}/", module.id)) {
                return Err(invalid(format!(
                    "module `{}` artifact `{path}` must stay under modules/{}/",
                    module.id, module.id
                )));
            }
        }
        for digest in [&module.manifest_sha256, &module.sha256] {
            if (require_release_fields || !digest.is_empty())
                && (digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
            {
                return Err(invalid(format!(
                    "module `{}` needs lowercase SHA-256 digests for its artifacts",
                    module.id
                )));
            }
        }
        match module.kind {
            PluginModuleKind::Source
                if !module.manifest.ends_with("/memcastle-source.toml")
                    || !module.entry.ends_with("/source.wasm") =>
            {
                return Err(invalid(format!(
                    "source module `{}` must declare its source manifest and source.wasm",
                    module.id
                )));
            }
            PluginModuleKind::Integration
                if !module.manifest.ends_with("/memcastle-integration.toml") =>
            {
                return Err(invalid(format!(
                    "integration module `{}` must declare its integration manifest",
                    module.id
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

fn version(value: &str, field: &str) -> Result<()> {
    semver::Version::parse(value)
        .map(|_| ())
        .map_err(|e| invalid(format!("{field} `{value}` is not a semantic version: {e}")))
}

fn requirement(value: &str, field: &str) -> Result<()> {
    semver::VersionReq::parse(value).map(|_| ()).map_err(|e| {
        invalid(format!(
            "{field} `{value}` is not a version requirement: {e}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PluginAuthentication, PluginDependency};

    type InvalidManifest = (&'static str, fn(&mut PluginManifest));

    fn fixture() -> PluginManifest {
        parse(&format!(
            "format = 1\nmemcastle = '>=0.4'\n[plugin]\nid = 'example'\nprovider = 'example'\nversion = '0.1.0'\ndescription = 'A provider'\nrepository = 'https://github.com/example/provider'\nlicense = 'MIT'\n[[modules]]\nid = 'source-one'\ntype = 'source'\nversion = '0.1.0'\nmanifest = 'modules/source-one/memcastle-source.toml'\nmanifest_sha256 = '{digest}'\nentry = 'modules/source-one/source.wasm'\nsha256 = '{digest}'\ncontract = '0.4.0'\n",
            digest = "0".repeat(64)
        ))
        .unwrap()
    }

    #[test]
    fn a_plugin_rejects_invalid_identity_release_and_shared_credentials() {
        let cases: &[InvalidManifest] = &[
            ("format 2 is unsupported", |m| m.format = 2),
            ("plugin.id", |m| m.plugin.id = "MixedCase".into()),
            ("plugin.provider", |m| m.plugin.provider = "bad/name".into()),
            ("plugin.version", |m| m.plugin.version = "latest".into()),
            ("memcastle", |m| m.memcastle = "not-a-range".into()),
            ("shared_config_schema", |m| {
                m.shared_config_schema = Some(serde_json::json!([]))
            }),
            ("authentication", |m| {
                m.authentication = Some(PluginAuthentication {
                    kind: "oauth_secret".into(),
                    credential: "source-one".into(),
                });
            }),
            ("plugin.description", |m| {
                m.plugin.description = "line one\nline two".into()
            }),
            ("plugin.repository", |m| {
                m.plugin.repository = "http://example.com".into()
            }),
        ];
        for &(reason, change) in cases {
            let mut manifest = fixture();
            change(&mut manifest);
            let error = validate(&manifest).unwrap_err().to_string();
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }

    #[test]
    fn dependencies_cannot_be_self_referential_duplicated_or_incompatible() {
        let mut manifest = fixture();
        manifest.dependencies.push(PluginDependency {
            id: manifest.plugin.id.clone(),
            version: ">=0.1".into(),
            optional: false,
        });
        assert!(
            validate(&manifest)
                .unwrap_err()
                .to_string()
                .contains("self-referential")
        );
        manifest.dependencies[0].id = "base".into();
        manifest.dependencies[0].version = "invalid".into();
        assert!(
            validate(&manifest)
                .unwrap_err()
                .to_string()
                .contains("dependency.version")
        );
        manifest.dependencies[0].version = ">=0.1".into();
        let duplicate = manifest.dependencies[0].clone();
        manifest.dependencies.push(duplicate);
        assert!(
            validate(&manifest)
                .unwrap_err()
                .to_string()
                .contains("duplicated")
        );
        manifest.dependencies.clear();
        manifest.dependencies = (0..65)
            .map(|i| PluginDependency {
                id: format!("plugin-{i}"),
                version: ">=0.1".into(),
                optional: false,
            })
            .collect();
        assert!(
            validate(&manifest)
                .unwrap_err()
                .to_string()
                .contains("at most 64")
        );
    }

    #[test]
    fn module_artifacts_must_be_unique_scoped_and_typed_before_packaging() {
        let cases: &[InvalidManifest] = &[
            ("module.version", |m| m.modules[0].version = "latest".into()),
            ("config_schema", |m| {
                m.modules[0].config_schema = Some(serde_json::json!([]))
            }),
            ("unsafe", |m| {
                m.modules[0].entry = "modules/source-one/../source.wasm".into()
            }),
            ("must stay under", |m| {
                m.modules[0].entry = "modules/other/source.wasm".into()
            }),
            ("SHA-256", |m| m.modules[0].sha256 = "unsigned".into()),
            ("source.wasm", |m| {
                m.modules[0].entry = "modules/source-one/source.txt".into()
            }),
            ("integration manifest", |m| {
                m.modules[0].kind = PluginModuleKind::Integration;
            }),
            ("duplicated", |m| {
                let duplicate = m.modules[0].clone();
                m.modules.push(duplicate);
            }),
        ];
        for &(reason, change) in cases {
            let mut manifest = fixture();
            change(&mut manifest);
            let error = validate(&manifest).unwrap_err().to_string();
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }

    #[test]
    fn a_source_and_an_integration_can_share_their_public_id() {
        let mut manifest = fixture();
        let mut integration = manifest.modules[0].clone();
        integration.kind = PluginModuleKind::Integration;
        integration.manifest = "modules/source-one/memcastle-integration.toml".into();
        integration.entry = "modules/source-one/dist/extension.js".into();
        integration.contract = None;
        manifest.modules.push(integration);
        validate(&manifest).unwrap();

        // A second integration with that ID would make its install receipt ambiguous.
        manifest.modules.push(manifest.modules[1].clone());
        assert!(
            validate(&manifest)
                .unwrap_err()
                .to_string()
                .contains("duplicated")
        );
    }

    #[test]
    fn archive_paths_have_one_unambiguous_filesystem_spelling() {
        for path in [
            "modules/assistant/./index.js",
            "modules//assistant/index.js",
            "modules/assistant/../index.js",
            "modules/assistant/index.js/",
            "/modules/assistant/index.js",
            "modules\\assistant\\index.js",
            "modules/assistant/dist/index.js:unreviewed",
        ] {
            assert!(
                !safe_path(path),
                "{path} must not alias another archive entry"
            );
        }
        assert!(safe_path("modules/assistant/dist/index.js"));
    }

    #[test]
    fn an_unknown_module_kind_is_not_installed_as_an_executable_source() {
        let text = toml::to_string(&fixture()).unwrap();
        let error =
            parse(&text.replace("type = \"source\"", "type = \"unknown_runtime\"")).unwrap_err();
        assert!(
            matches!(error, Error::PluginManifestInvalid { .. }),
            "{error}"
        );
    }
}
