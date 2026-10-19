//! Exercise a reviewed plugin archive against a disposable daemon, never a configured palace.

use std::collections::{BTreeMap, HashSet};
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::app::MinerPatch;
use crate::domain::{PluginModule, PluginModuleKind, SourceState};
use crate::error::{Error, Result};
use crate::integration::Catalog;
use crate::plugin::package;

use super::DaemonClient;

fn failed(name: &str, reason: impl Into<String>) -> Error {
    Error::PluginBlocked {
        name: name.into(),
        reason: reason.into(),
    }
}

fn ensure(condition: bool, name: &str, reason: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(failed(name, reason))
    }
}

fn inventory(modules: &[PluginModule]) -> HashSet<(PluginModuleKind, String)> {
    modules
        .iter()
        .map(|module| (module.kind, module.id.clone()))
        .collect()
}

/// Preview, install and enable an exact archive in private ephemeral state.
///
/// # Errors
///
/// Refuses malformed archives, unavailable daemons and lifecycle mismatches.
pub async fn run(archive: &Path) -> Result<()> {
    let bytes = std::fs::read(archive).map_err(|e| Error::io(archive.display().to_string(), e))?;
    let package = package::unpack(&bytes)?;
    let id = package.manifest.plugin.id.clone();
    let scratch = tempfile::tempdir().map_err(|e| Error::io("plugin test scratch directory", e))?;
    let palace = scratch.path().join("palace");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|e| Error::io("plugin test loopback port", e))?;
    let address = listener
        .local_addr()
        .map_err(|e| Error::io("plugin test loopback port", e))?;
    drop(listener);

    let binary = std::env::current_exe().map_err(|e| Error::io("memcastle executable", e))?;
    let mut command = Command::new(binary);
    command
        .arg("--palace")
        .arg(&palace)
        .arg("serve")
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(address.port().to_string())
        .current_dir(scratch.path())
        // The test must never read the user's token, home, config or palace.
        .env_clear()
        .env("HOME", scratch.path())
        .env("XDG_CONFIG_HOME", scratch.path().join("config"))
        .env("XDG_DATA_HOME", scratch.path().join("data"))
        .env("XDG_STATE_HOME", scratch.path().join("state"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // Windows needs its system directory to load native libraries even for a child with isolated user state.
        for key in ["SYSTEMROOT", "WINDIR", "COMSPEC", "PATHEXT"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command.env("USERPROFILE", scratch.path());
        command.env("TEMP", scratch.path());
        command.env("TMP", scratch.path());
    }
    let mut child = command
        .spawn()
        .map_err(|e| Error::io("start disposable plugin test daemon", e))?;

    let client = DaemonClient::discover(&palace, address);
    let result = async {
        let mut ready = false;
        for _ in 0..120 {
            if let Some(status) = child
                .try_wait()
                .map_err(|e| Error::io("plugin test daemon", e))?
            {
                let mut details = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut details).await;
                }
                return Err(failed(
                    &id,
                    format!(
                        "disposable daemon exited during startup: {status}: {}",
                        details.trim()
                    ),
                ));
            }
            if client.health().await {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        ensure(
            ready,
            &id,
            "disposable daemon was not ready within 30 seconds",
        )?;
        let preview = client.preview_plugin(&bytes).await?;
        ensure(
            preview.id == id
                && preview.version == package.manifest.plugin.version
                && preview.adoptions.is_empty(),
            &id,
            "archive preview differs from its manifest or unexpectedly requires adoption",
        )?;
        ensure(
            inventory(&preview.modules) == inventory(&package.manifest.modules),
            &id,
            "preview changed the plugin's typed module inventory",
        )?;
        ensure(
            !client
                .list_plugins()
                .await?
                .iter()
                .any(|item| item.id == id),
            &id,
            "disposable palace already contains this plugin",
        )?;
        let consents = preview
            .source_consents
            .iter()
            .chain(&preview.embedding_consents)
            .map(|source| (source.source.clone(), source.digest.clone()))
            .collect::<BTreeMap<_, _>>();
        let installed = client.install_plugin(bytes.clone(), &consents, &[]).await?;
        ensure(
            inventory(&installed.manifest.modules) == inventory(&package.manifest.modules),
            &id,
            "installed plugin has a different module inventory",
        )?;
        let shown = client.show_plugin(&id).await?;
        ensure(
            shown.digest == installed.digest,
            &id,
            "installed plugin digest changed after installation",
        )?;
        let integrations = package
            .manifest
            .modules
            .iter()
            .filter(|module| module.kind == PluginModuleKind::Integration)
            .map(|module| module.id.as_str())
            .collect::<HashSet<_>>();
        // The agent-side catalogue reads installed generations, not the daemon's plugin inventory.
        let catalogs = Catalog::plugin_catalogs(&scratch.path().join("data/memcastle/plugins"))?;
        let discovered = catalogs
            .iter()
            .flat_map(|catalog| catalog.integrations().iter().map(|item| item.id()))
            .collect::<HashSet<_>>();
        ensure(
            discovered == integrations,
            &id,
            "installed integrations differ from the archive's module inventory",
        )?;
        for module in &package.manifest.modules {
            if module.kind == PluginModuleKind::Source {
                let source = client.show_source(&module.id).await?;
                ensure(
                    source.state == SourceState::Installed,
                    &id,
                    format!("source `{}` was activated during install", module.id),
                )?;
                let enabled = client.set_source_enabled(&module.id, true).await?;
                ensure(
                    enabled.state == SourceState::Enabled,
                    &id,
                    format!("source `{}` could not be enabled", module.id),
                )?;
            }
        }
        // Replaying the exact release must not undo an explicitly enabled source.
        let replayed = client.install_plugin(bytes.clone(), &consents, &[]).await?;
        ensure(
            replayed.digest == installed.digest,
            &id,
            "reinstalling identical bytes changed the installed plugin",
        )?;

        // A new parent release with identical real module bytes tests the upgrade path without inventing guest code.
        let mut next = package.manifest.clone();
        let mut version = semver::Version::parse(&next.plugin.version)
            .map_err(|e| failed(&id, format!("invalid packaged version: {e}")))?;
        version.patch += 1;
        version.pre = semver::Prerelease::EMPTY;
        version.build = semver::BuildMetadata::EMPTY;
        next.plugin.version = version.to_string();
        let mut next_files = package.files.clone();
        next_files.insert(
            "plugin.toml".into(),
            toml::to_string(&next)
                .map_err(|e| Error::serialization("plugin test upgrade descriptor", e))?
                .into_bytes(),
        );
        next_files.remove("manifest.toml");
        let upgraded_bytes = package::pack(&next_files)?;
        let upgraded_preview = client.preview_plugin(&upgraded_bytes).await?;
        ensure(
            inventory(&upgraded_preview.modules) == inventory(&package.manifest.modules),
            &id,
            "upgrading changed the plugin's typed module inventory",
        )?;
        let upgraded = client
            .install_plugin(upgraded_bytes.clone(), &consents, &[])
            .await?;
        ensure(
            upgraded.manifest.plugin.version == next.plugin.version,
            &id,
            "upgrade did not install the new parent version",
        )?;
        for module in &package.manifest.modules {
            if module.kind == PluginModuleKind::Source {
                ensure(
                    client.show_source(&module.id).await?.state == SourceState::Enabled,
                    &id,
                    format!("upgrading disabled source `{}`", module.id),
                )?;
            }
        }

        if let Some(source) = package
            .manifest
            .modules
            .iter()
            .find(|module| module.kind == PluginModuleKind::Source)
        {
            client
                .set_miner(
                    "plugin-test-miner",
                    &MinerPatch {
                        source: Some(source.id.clone()),
                        enabled: Some(false),
                        ..MinerPatch::default()
                    },
                )
                .await?;
            ensure(
                client.uninstall_plugin(&id).await.is_err(),
                &id,
                "uninstall removed a plugin still referenced by a disabled miner",
            )?;
            ensure(
                client.show_miner("plugin-test-miner").await?.source == source.id,
                &id,
                "a blocked uninstall changed the configured miner",
            )?;
            client.remove_miner("plugin-test-miner").await?;
        }
        for module in &package.manifest.modules {
            if module.kind == PluginModuleKind::Source {
                client.set_source_enabled(&module.id, false).await?;
            }
        }
        client.uninstall_plugin(&id).await?;
        ensure(
            !client
                .list_plugins()
                .await?
                .iter()
                .any(|item| item.id == id),
            &id,
            "uninstall left the plugin installed",
        )?;
        let restored = client
            .install_plugin(upgraded_bytes, &consents, &[])
            .await?;
        ensure(
            inventory(&restored.manifest.modules) == inventory(&package.manifest.modules),
            &id,
            "reinstall after uninstall lost plugin modules",
        )?;
        Ok(())
    }
    .await;
    // Dropping the temporary palace before the child exits can leave its embedded DB locked.
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}
