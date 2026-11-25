//! Run-scoped immutable databases, copied into private test palaces.
#![allow(dead_code)] // Shared by the library unit-test and integration-test harnesses.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

pub struct RunCache {
    pub root: PathBuf,
}

/// Nextest processes share immutable templates, but none owns whole-run teardown.
#[rstest::fixture]
pub fn run_cache() -> Option<RunCache> {
    let run = std::env::var_os("NEXTEST_RUN_ID")?;
    let workspace = fs::canonicalize(env!("CARGO_MANIFEST_DIR"))
        .expect("resolve the fixture workspace before preparing databases");
    let mut identity = Sha256::new();
    identity.update(workspace.as_os_str().as_encoded_bytes());
    identity.update([0]);
    identity.update(run.as_encoded_bytes());
    let root = std::env::temp_dir()
        .join("memcastle-test-fixtures-v2")
        .join(cache_key(identity));
    fs::create_dir_all(&root).expect("create the run's temporary database cache");
    Some(RunCache { root })
}

/// Generate once across nextest processes; a failed producer never publishes.
pub fn prepare(
    cache: &RunCache,
    kind: &str,
    executable: &Path,
    producer: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let executable = fs::canonicalize(executable)?;
    let metadata = fs::metadata(&executable)?;
    // A single run may use several build artifacts; filename hashes alone do
    // not distinguish a binary rebuilt in place. Metadata avoids hashing GBs.
    let mut identity = Sha256::new();
    identity.update(executable.as_os_str().as_encoded_bytes());
    identity.update(metadata.len().to_le_bytes());
    identity.update(format!("{:?}", metadata.modified()?));
    let root = cache.root.join(cache_key(identity));
    fs::create_dir_all(&root)?;
    let published = root.join(kind);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(format!("{kind}.lock")))?;
    // OS locks are released even when a producer crashes; a sentinel file
    // would leave the rest of a run waiting on a dead test process.
    lock.lock()?;
    if !published.exists() {
        let staging = tempfile::Builder::new()
            .prefix("preparing-")
            .tempdir_in(&root)?;
        let database = staging.path().join("db");
        producer(&database)?;
        if !database.is_dir() || fs::read_dir(&database)?.next().is_none() {
            return Err(io::Error::other(
                "fixture producer succeeded without creating a database",
            ));
        }
        fs::rename(staging.path(), &published)?;
        // Later test processes still need this template, so publication must
        // outlive the process that won the preparation lock.
        let _ = staging.keep();
    }
    let database = published.join("db");
    if !database.is_dir() {
        return Err(io::Error::other("published database fixture is incomplete"));
    }
    Ok(database)
}

fn cache_key(identity: Sha256) -> String {
    // SHA-256's fixed-size output does not implement LowerHex in sha2 0.11.
    identity
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Copy bytes, never writable file inodes or an open datastore.
pub fn copy_database(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_database(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::other(
                "database fixture contains a non-regular file",
            ));
        }
    }
    Ok(())
}

/// Keep mutable unit-test palaces alive until their SDK processes have exited.
pub fn private_copy(cache: &RunCache, source: &Path) -> io::Result<PathBuf> {
    let directory = tempfile::Builder::new()
        .prefix("unit-")
        .tempdir_in(&cache.root)?;
    let database = directory.path().join("db");
    copy_database(source, &database)?;
    // The SDK may still have asynchronous teardown tasks after its handle drops.
    // Retain private unit copies for deferred cleanup with the run's templates.
    let _ = directory.keep();
    Ok(database)
}

/// Test-owned configuration cannot inherit a developer's palace or secrets.
pub fn isolate(command: &mut Command, directory: &Path) {
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy().to_ascii_uppercase();
        // A re-executed producer must not discover its own nextest fixture and
        // recursively wait on the lock its parent holds. Keep LLVM coverage env.
        if name.starts_with("MEMCASTLE_") || name.starts_with("NEXTEST_") {
            command.env_remove(key);
        }
    }
    command
        .env("XDG_CONFIG_HOME", directory.join("config"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("XDG_STATE_HOME", directory.join("state"))
        .env("MEMCASTLE_STORE_SYNC", "every");
}

/// Bound a child and keep its diagnostics without a pipe that can fill up.
pub fn finish(command: &mut Command, directory: &Path) -> io::Result<()> {
    let stdout_path = directory.join("producer.stdout");
    let stderr_path = directory.join("producer.stderr");
    command
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout_path)?)
        .stderr(fs::File::create(&stderr_path)?);
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        if let Some(status) = status {
            if status.success() {
                return Ok(());
            }
            return Err(io::Error::other(format!(
                "database fixture producer failed ({status}):\n{}\n{}",
                fs::read_to_string(stdout_path)?,
                fs::read_to_string(stderr_path)?
            )));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "database fixture producer exceeded 90s",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
