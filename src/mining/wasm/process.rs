//! The `run-process` host function: how a source runs a program it was granted, and nothing else.
//!
//! The granted names are matched exactly, there is no shell, the child sees only the environment the manifest listed
//! (plus `PATH`, to be found), and the run is bounded in time and in output.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

/// The most output of either stream a program may produce. A source has a memory limit of its own; this keeps one
/// chatty program from filling it with a single answer.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;

/// What a program produced.
#[derive(Debug)]
pub(super) struct Output {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Why a program was not run or did not finish.
#[derive(Debug)]
pub(super) enum ProcessError {
    /// The program is not in the source's permissions. Reported to the user as a permission error, not a failure.
    Denied(String),
    /// It was permitted but could not be run to completion.
    Failed(String),
}

/// What a source may run, and with what.
#[derive(Debug, Clone, Default)]
pub(super) struct ProcessGrant {
    /// The program names that may be run.
    pub allowed: Vec<String>,
    /// The environment variables the child may see, resolved from the daemon's own environment.
    pub env: Vec<(String, String)>,
    /// Do not implicitly inherit the selected credential even when its reference is PATH or HOME.
    pub excluded_env: Option<String>,
    /// How long one run may take.
    pub timeout: Duration,
    /// Acquisition already spent part of its budget before asking for a program.
    pub deadline: Option<Instant>,
    /// A dropped host call must not leave a blocking worker running its child.
    pub cancelled: CancellationToken,
}

/// An anonymous scratch file for one of a program's standard streams.
fn scratch_file(program: &str) -> Result<File, ProcessError> {
    tempfile::tempfile().map_err(|source| {
        ProcessError::Failed(format!(
            "no scratch file could be created to capture the output of `{program}`: {source}; check that the \
             temporary directory (`TMPDIR`) exists and has free space"
        ))
    })
}

/// A second handle on `file` for the child to write through; both share one offset and one length.
fn clone_file(file: &File, program: &str) -> Result<File, ProcessError> {
    file.try_clone().map_err(|source| {
        ProcessError::Failed(format!(
            "the output of `{program}` could not be captured: {source}"
        ))
    })
}

/// Everything the program wrote to `file`, up to one byte past the cap (so the caller can tell it was exceeded).
fn read_back(file: &mut File, program: &str) -> Result<Vec<u8>, ProcessError> {
    let failed = |source: std::io::Error| {
        ProcessError::Failed(format!(
            "the output of `{program}` could not be read back: {source}"
        ))
    };
    // The handle shares its offset with the child's, which is at the end of what it wrote.
    file.seek(SeekFrom::Start(0)).map_err(failed)?;
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(failed)?;
    Ok(bytes)
}

/// Run `program` if `grant` allows it.
pub(super) fn run(
    grant: &ProcessGrant,
    program: &str,
    args: &[String],
    stdin: Option<&[u8]>,
) -> Result<Output, ProcessError> {
    if !grant.allowed.iter().any(|allowed| allowed == program) {
        return Err(ProcessError::Denied(format!(
            "running `{program}` is not permitted; its manifest allows: {}",
            if grant.allowed.is_empty() {
                "nothing".to_string()
            } else {
                grant.allowed.join(", ")
            }
        )));
    }
    let deadline = grant
        .deadline
        .unwrap_or_else(|| Instant::now() + grant.timeout);
    let check_live = || {
        if grant.cancelled.is_cancelled() || Instant::now() >= deadline {
            Err(ProcessError::Failed(format!(
                "`{program}` was not started because the source call has ended"
            )))
        } else {
            Ok(())
        }
    };
    check_live()?;
    // Output goes to anonymous files, not pipes: a CLI built on Bun (OpenCode) can exit before a large write to a pipe
    // has drained, so the answer is cut at a multiple of 64 KiB while the exit status is still 0, and a source then
    // parses half a document. A file is written synchronously, so what the program printed is what we read. The files
    // are unlinked on creation: there is no path to leak and nothing to clean up, and they are the host's scratch, not
    // something the guest is granted.
    let mut stdout_file = scratch_file(program)?;
    let mut stderr_file = scratch_file(program)?;
    // A descendant can keep a stdin pipe open after its parent exits, leaving a writer thread unjoinable.
    // The input is already a complete byte buffer, so an anonymous file preserves it without a blocking writer.
    let input = if let Some(bytes) = stdin {
        let mut file = scratch_file(program)?;
        for chunk in bytes.chunks(64 * 1024) {
            // Large inputs must not keep being copied after the invocation has ended.
            check_live()?;
            file.write_all(chunk).map_err(|source| {
                ProcessError::Failed(format!(
                    "the input of `{program}` could not be prepared: {source}"
                ))
            })?;
        }
        file.rewind().map_err(|source| {
            ProcessError::Failed(format!(
                "the input of `{program}` could not be rewound: {source}"
            ))
        })?;
        Stdio::from(file)
    } else {
        Stdio::null()
    };
    // If PATH itself is the selected credential, resolve the granted program on the host first,
    // rather than passing the credential on to the child just so Command can find its executable.
    let executable = if grant.excluded_env.as_deref() == Some("PATH")
        && !grant.env.iter().any(|(name, _)| name == "PATH")
    {
        std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths).find_map(|dir| {
                    let path = dir.join(program);
                    if path.is_file() {
                        return Some(path);
                    }
                    #[cfg(windows)]
                    if path.with_extension("exe").is_file() {
                        return Some(path.with_extension("exe"));
                    }
                    None
                })
            })
            .ok_or_else(|| {
                ProcessError::Failed(format!(
                    "`{program}` could not be found on the daemon's PATH"
                ))
            })?
    } else {
        PathBuf::from(program)
    };
    let mut command = Command::new(executable);
    command
        .args(args)
        .env_clear()
        .stdin(input)
        .stdout(Stdio::from(clone_file(&stdout_file, program)?))
        .stderr(Stdio::from(clone_file(&stderr_file, program)?));
    // `PATH` so the program is found, `HOME` (and `SystemRoot` on Windows) so tools find their own configuration.
    // No other variable is inherited, so a token in the daemon's environment is not one a granted `git` can read
    // unless the manifest asked for it.
    for inherited in ["PATH", "SystemRoot", "HOME"] {
        if grant.excluded_env.as_deref() == Some(inherited) {
            continue;
        }
        if let Ok(value) = std::env::var(inherited) {
            command.env(inherited, value);
        }
    }
    command.envs(grant.env.iter().map(|(k, v)| (k, v)));

    check_live()?;
    let mut child = command.spawn().map_err(|source| {
        ProcessError::Failed(format!("`{program}` could not be started: {source}"))
    })?;
    let too_much = || {
        ProcessError::Failed(format!(
            "`{program}` produced more than {} MiB of output",
            MAX_OUTPUT_BYTES / 1024 / 1024
        ))
    };

    let status = loop {
        // Dropping a spawn_blocking handle does not stop it; the worker must kill and reap its own child.
        if grant.cancelled.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ProcessError::Failed(format!(
                "`{program}` was cancelled and was stopped"
            )));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProcessError::Failed(format!(
                    "`{program}` did not finish within {}s and was stopped",
                    grant.timeout.as_secs()
                )));
            }
            // A file, unlike a pipe, does not make a chatty program wait for a reader, so the cap is enforced while it
            // runs: otherwise an endless writer would fill the disk until the time limit.
            Ok(None)
                if [&stdout_file, &stderr_file]
                    .iter()
                    .any(|file| file.metadata().is_ok_and(|m| m.len() > MAX_OUTPUT_BYTES)) =>
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(too_much());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProcessError::Failed(format!(
                    "waiting for `{program}` failed: {source}"
                )));
            }
        }
    };
    let stdout = read_back(&mut stdout_file, program)?;
    let stderr = read_back(&mut stderr_file, program)?;
    // The program may have written its last bytes between the final size check and its exit.
    if stdout.len() as u64 > MAX_OUTPUT_BYTES || stderr.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(too_much());
    }
    Ok(Output {
        status: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn grant(allowed: &[&str]) -> ProcessGrant {
        ProcessGrant {
            allowed: allowed.iter().map(|s| (*s).to_string()).collect(),
            env: vec![],
            excluded_env: None,
            timeout: Duration::from_secs(5),
            ..ProcessGrant::default()
        }
    }

    #[test]
    fn a_program_outside_the_grant_is_denied_not_run() {
        let error = run(&grant(&["echo"]), "cat", &[], None).unwrap_err();
        assert!(matches!(error, ProcessError::Denied(_)), "{error:?}");
        assert!(matches!(
            run(&grant(&[]), "echo", &[], None),
            Err(ProcessError::Denied(_))
        ));
    }

    #[test]
    fn a_selected_path_reference_is_not_inherited_by_a_host_run_program() {
        let mut granted = grant(&["env"]);
        granted.excluded_env = Some("PATH".into());
        let output = run(&granted, "env", &[], None).unwrap();
        let lines = String::from_utf8(output.stdout).unwrap();
        assert!(
            !lines.lines().any(|line| line.starts_with("PATH=")),
            "{lines}"
        );
        // A separately declared manifest env grant is deliberate rather than ambient inheritance.
        let path = std::env::var("PATH").unwrap();
        granted.env.push(("PATH".into(), path.clone()));
        let output = run(&granted, "env", &[], None).unwrap();
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains(&format!("PATH={path}"))
        );
    }

    #[test]
    fn a_granted_program_runs_without_a_shell_and_its_output_comes_back() {
        // Shell metacharacters are plain arguments: nothing interprets them.
        let output = run(&grant(&["echo"]), "echo", &["a; echo b".to_string()], None).unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "a; echo b");
        assert_eq!(output.status, 0);
    }

    #[test]
    fn standard_input_reaches_the_program() {
        let output = run(&grant(&["cat"]), "cat", &[], Some(b"hello")).unwrap();
        assert_eq!(output.stdout, b"hello");
    }

    #[test]
    fn a_descendant_inheriting_stdin_does_not_hold_the_worker_open() {
        // More than a pipe can buffer; the descendant deliberately never consumes its inherited input.
        let input = vec![b'x'; 2 * 1024 * 1024];
        let started = Instant::now();
        let output = run(
            &grant(&["sh"]),
            "sh",
            &[
                "-c".into(),
                "sleep 5 <&0 >/dev/null 2>&1 & printf '%s' \"$!\"".into(),
            ],
            Some(&input),
        )
        .unwrap();
        let pid = String::from_utf8(output.stdout).unwrap();
        // The descendant is intentionally outside the direct-child handle; clean up this test's helper explicitly.
        let _ = Command::new("kill").arg(pid).stderr(Stdio::null()).status();
        assert_eq!(output.status, 0);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the worker waited for the descendant's stdin"
        );
    }

    #[test]
    fn the_child_sees_only_the_environment_it_was_granted() {
        let mut granted = grant(&["env"]);
        granted.env = vec![("ALLOWED".into(), "yes".into())];
        let output = run(&granted, "env", &[], None).unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("ALLOWED=yes"), "{text}");
        assert!(!text.contains("CARGO"), "{text}");
    }

    #[test]
    fn a_program_that_outlives_the_time_limit_is_stopped() {
        let mut granted = grant(&["sleep"]);
        granted.timeout = Duration::from_millis(100);
        let error = run(&granted, "sleep", &["5".to_string()], None).unwrap_err();
        assert!(
            matches!(error, ProcessError::Failed(ref m) if m.contains("did not finish")),
            "{error:?}"
        );
    }

    #[test]
    fn cancelling_a_blocking_worker_stops_and_reaps_its_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("child.pid");
        let granted = grant(&["sh"]);
        let cancelled = granted.cancelled.clone();
        // Also stop the child if an assertion fails before the explicit cancellation.
        let _cancel_on_drop = cancelled.clone().drop_guard();
        let argument = pid_file.display().to_string();
        let worker = std::thread::spawn(move || {
            run(
                &granted,
                "sh",
                &[
                    "-c".into(),
                    "printf '%s' \"$$\" > \"$1\"; exec sleep 10".into(),
                    "process-cancellation".into(),
                    argument,
                ],
                None,
            )
        });
        let wait_until = Instant::now() + Duration::from_secs(2);
        let pid = loop {
            if let Ok(pid) = std::fs::read_to_string(&pid_file)
                && !pid.is_empty()
            {
                break Some(pid);
            }
            if Instant::now() >= wait_until {
                break None;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        cancelled.cancel();
        let result = worker.join().unwrap();
        let pid = pid.expect("the child never started");
        assert!(
            matches!(result, Err(ProcessError::Failed(ref message)) if message.contains("cancelled"))
        );
        assert!(
            !Command::new("kill")
                .args(["-0", &pid])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success(),
            "a cancelled worker left its native child alive or unreaped"
        );
    }

    #[test]
    fn a_granted_program_that_does_not_exist_fails_to_start_and_is_not_a_denial() {
        let error = run(
            &grant(&["memcastle-no-such-program"]),
            "memcastle-no-such-program",
            &[],
            None,
        )
        .unwrap_err();
        assert!(
            matches!(error, ProcessError::Failed(ref m) if m.contains("could not be started")),
            "{error:?}"
        );
    }

    #[test]
    fn more_output_than_the_cap_is_an_error_not_an_unbounded_answer() {
        // 17 MiB of zeros, a MiB over the cap.
        let error = run(
            &grant(&["head"]),
            "head",
            &[
                "-c".to_string(),
                (17 * 1024 * 1024).to_string(),
                "/dev/zero".to_string(),
            ],
            None,
        )
        .unwrap_err();
        assert!(
            matches!(error, ProcessError::Failed(ref m) if m.contains("more than 16 MiB")),
            "{error:?}"
        );
    }

    #[test]
    fn a_large_answer_comes_back_whole_and_so_does_standard_error() {
        // Well past a pipe's 64 KiB buffer, which is where a CLI that exits early used to be cut off.
        let output = run(
            &grant(&["sh"]),
            "sh",
            &[
                "-c".to_string(),
                "head -c 2097152 /dev/zero; echo oops >&2".to_string(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(output.stdout.len(), 2 * 1024 * 1024);
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "oops");
    }

    #[test]
    fn a_program_that_writes_without_end_is_stopped_at_the_cap_not_at_the_time_limit() {
        let mut granted = grant(&["yes"]);
        granted.timeout = Duration::from_secs(60);
        let started = Instant::now();
        let error = run(&granted, "yes", &[], None).unwrap_err();
        assert!(
            matches!(error, ProcessError::Failed(ref m) if m.contains("more than 16 MiB")),
            "{error:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the cap was enforced only by the time limit"
        );
    }

    #[test]
    fn a_program_that_fails_reports_its_exit_status_rather_than_an_error() {
        let output = run(&grant(&["false"]), "false", &[], None).unwrap();
        assert_ne!(output.status, 0);
    }
}
