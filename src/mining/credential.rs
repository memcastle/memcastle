//! Resolve an operator-selected source credential without putting its value in job state.

use std::io::{BufRead, Read};

use crate::domain::{CredentialRef, Secret};

/// Read the selected reference at execution time, including after a resumed job.
/// Errors deliberately contain only the reference and the action to take, never the contents.
pub(crate) fn resolve(reference: &CredentialRef) -> Result<Secret, String> {
    match reference {
        CredentialRef::Env { name } => match std::env::var(name) {
            Ok(value) if !value.is_empty() => Ok(Secret::new(value)),
            _ => Err(format!(
                "credential environment variable `{name}` is missing, empty or invalid UTF-8; set it in the daemon's environment and restart the daemon"
            )),
        },
        CredentialRef::File { path } => {
            let file = std::fs::File::open(path).map_err(|_| {
                format!("credential file `{path}` is missing or unreadable; create a readable file")
            })?;
            let mut line = Vec::new();
            // A bounded first line prevents accidentally loading an arbitrary file into memory.
            let mut reader = std::io::BufReader::new(file).take(64 * 1024 + 1);
            reader.read_until(b'\n', &mut line).map_err(|_| {
                format!("credential file `{path}` cannot be read; check its permissions")
            })?;
            if line.len() > 64 * 1024 {
                return Err(format!(
                    "credential file `{path}` has a first line longer than 64 KiB; shorten it"
                ));
            }
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let value = String::from_utf8(line).map_err(|_| {
                format!("credential file `{path}` has an invalid UTF-8 first line; replace it")
            })?;
            if value.is_empty() {
                return Err(format!(
                    "credential file `{path}` has an empty first line; fill it"
                ));
            }
            Ok(Secret::new(value))
        }
        CredentialRef::Oauth => {
            Err("OAuth credentials use `host.access-token`, not `host.source-credential`".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_credential_reads_only_its_first_line_and_never_prints_its_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential");
        std::fs::write(&path, "first-secret\nsecond-secret\n").unwrap();
        let reference = CredentialRef::File {
            path: path.display().to_string(),
        };
        let secret = resolve(&reference).unwrap();
        assert_eq!(secret.expose(), "first-secret");
        assert!(!format!("{secret:?}").contains("first-secret"));
        std::fs::write(&path, [0xff, b'\n']).unwrap();
        let error = resolve(&reference).unwrap_err();
        assert!(error.contains("invalid UTF-8"));
        assert!(!error.contains("first-secret"));
        std::fs::write(&path, "\n").unwrap();
        assert!(
            resolve(&reference)
                .unwrap_err()
                .contains("empty first line")
        );
    }

    #[test]
    fn a_missing_reference_names_only_the_location_and_action() {
        let reference = CredentialRef::File {
            path: "/nonexistent/source-credential".into(),
        };
        let message = resolve(&reference).unwrap_err();
        assert!(message.contains("create a readable file"));
        assert!(message.contains("/nonexistent/source-credential"));
    }

    #[test]
    fn an_environment_reference_reads_the_daemons_current_value() {
        // PATH is provided to the test daemon by every supported operating system's process launcher.
        let reference = CredentialRef::Env {
            name: "PATH".into(),
        };
        assert_eq!(
            resolve(&reference).unwrap().expose(),
            std::env::var("PATH").unwrap()
        );
        let absent = CredentialRef::Env {
            name: "MEMCASTLE_SOURCE_CREDENTIAL_NOT_SET_BY_TESTS".into(),
        };
        assert!(
            resolve(&absent)
                .unwrap_err()
                .contains("set it in the daemon's environment")
        );
    }
}
