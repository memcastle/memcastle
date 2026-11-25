//! Run-scoped immutable databases, copied into private test palaces.
#![allow(dead_code)] // Shared by the library unit-test and integration-test harnesses.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The runner owns this directory and removes it after every test child exits.
pub fn root() -> Option<PathBuf> {
    std::env::var_os("MEMCASTLE_TEST_FIXTURE_ROOT").map(|root| {
        let root = PathBuf::from(root);
        assert!(
            root.is_absolute() && root.is_dir(),
            "invalid prepared-fixture root: {}",
            root.display()
        );
        assert_eq!(
            fs::read_to_string(root.join(".owner")).ok().as_deref(),
            Some("memcastle-test-fixtures-v1\n"),
            "prepared fixtures require the test runner owner"
        );
        root
    })
}

/// Generate once across nextest processes; a failed producer never publishes.
pub fn prepare(
    kind: &str,
    producer: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<Option<PathBuf>> {
    let Some(root) = root() else { return Ok(None) };
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
        // Rename publishes the whole completed fixture atomically. Its lifetime
        // belongs to the runner, not the process that won the preparation lock.
        let _ = staging.keep();
    }
    let database = published.join("db");
    if !database.is_dir() {
        return Err(io::Error::other("published database fixture is incomplete"));
    }
    Ok(Some(database))
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
pub fn private_copy(source: &Path) -> io::Result<PathBuf> {
    let root = root().expect("a prepared database requires a fixture owner");
    let directory = tempfile::Builder::new().prefix("unit-").tempdir_in(root)?;
    let database = directory.path().join("db");
    copy_database(source, &database)?;
    let _ = directory.keep();
    Ok(database)
}

/// Test-owned configuration cannot inherit a developer's palace or secrets.
pub fn isolate(command: &mut Command, directory: &Path) {
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("MEMCASTLE_")
        {
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
