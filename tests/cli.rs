//! CLI smoke tests: the binary parses its documented commands and behaves
//! sensibly with no daemon around.

use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

#[cfg(target_os = "linux")]
#[test]
fn slow_cli_requests_show_feedback_without_corrupting_json_or_errors() {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::process::{Command as ProcessCommand, Stdio};
    use std::time::Duration;

    for (args, json, failed, expected_path, message) in [
        (
            vec!["plugin", "list"],
            false,
            false,
            "/api/plugins",
            "Listing installed plugins...",
        ),
        (
            vec!["plugin", "list"],
            true,
            false,
            "/api/plugins",
            "Listing installed plugins...",
        ),
        (
            vec!["sources"],
            false,
            true,
            "/api/sources",
            "Listing sources...",
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config.toml");
        std::fs::write(
            &config,
            format!(
                "[palace]\npath = {:?}\n[server]\nbind = '127.0.0.1'\nport = {}\n",
                temp.path().join("palace").display().to_string(),
                listener.local_addr().unwrap().port()
            ),
        )
        .unwrap();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("accepting plugin request: {error}"),
                }
            };
            let mut request = [0; 4096];
            let bytes = stream.read(&mut request).unwrap();
            assert!(
                String::from_utf8_lossy(&request[..bytes])
                    .contains(&format!("GET {expected_path}"))
            );
            // Hold the response long enough for the spinner's steady tick to paint a frame.
            std::thread::sleep(Duration::from_millis(250));
            if failed {
                stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: 29\r\nConnection: close\r\n\r\n{\"error\":\"temporary failure\"}").unwrap();
            } else {
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]").unwrap();
            }
        });

        let mut master_fd = -1;
        let mut slave_fd = -1;
        // SAFETY: openpty initializes two distinct descriptors and File owns each once.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &raw mut master_fd,
                    &raw mut slave_fd,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0
        );
        let mut master = unsafe { File::from_raw_fd(master_fd) };
        let slave = unsafe { File::from_raw_fd(slave_fd) };
        let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_memcastle"));
        command
            .args(["--config", config.to_str().unwrap()])
            .args(args)
            // indicatif intentionally hides progress when CI leaves TERM unset or sets it to dumb.
            .env("TERM", "xterm-256color")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(slave));
        if json {
            command.arg("--json");
        }
        for (name, _) in std::env::vars().filter(|(name, _)| name.starts_with("MEMCASTLE_")) {
            command.env_remove(name);
        }
        let output = command.output().unwrap();
        server.join().unwrap();
        assert_eq!(output.status.success(), !failed, "{output:?}");
        if failed {
            assert!(output.stdout.is_empty(), "{output:?}");
        } else {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
                serde_json::json!([])
            );
        }
        let mut terminal = String::new();
        // No child holds the slave now. Linux reports EIO instead of EOF for a closed PTY.
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            0
        );
        match master.read_to_string(&mut terminal) {
            Ok(_) => {}
            Err(error) if error.raw_os_error() == Some(libc::EIO) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("reading pseudo-terminal: {error}"),
        }
        if json {
            assert!(
                terminal.is_empty(),
                "JSON mode wrote progress: {terminal:?}"
            );
        } else {
            assert!(terminal.contains(message), "{terminal:?}");
            if failed {
                assert!(terminal.contains("temporary failure"), "{terminal:?}");
            }
        }
    }
}

#[test]
fn help_lists_the_top_level_commands() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("serve"))
        .stdout(contains("daemon"))
        .stdout(contains("mine"))
        .stdout(contains("audit"))
        .stdout(contains("repair"))
        .stdout(contains("  job "))
        .stdout(contains("recall"))
        .stdout(contains("  note "))
        .stdout(contains("wake-up"));
}

#[test]
fn tui_help_is_available_without_a_daemon() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["tui", "--help"])
        .assert()
        .success()
        .stdout(contains("interactive operations console"));
}

#[test]
fn tui_refuses_piped_input_instead_of_entering_raw_mode() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .arg("tui")
        .assert()
        .failure()
        .stderr(contains("interactive stdin and stdout are required"));
}

#[test]
fn tui_refuses_json_mode() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["tui", "--json"])
        .assert()
        .failure()
        .stderr(contains("--json cannot be used"));
}

#[test]
fn doctor_help_lists_the_offline_diagnostics_command() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(contains("Diagnose local configuration"));
}

#[test]
fn doctor_reports_invalid_config_without_printing_secret_values() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    std::fs::write(
        &config,
        "[embeddings]\nprovider = 'secret-redaction-canary'\n",
    )
    .unwrap();
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .args(["doctor", "--config", config.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\": \"error\""), "{stdout}");
    assert!(stdout.contains("\"status\": \"skipped\""), "{stdout}");
    assert!(!stdout.contains("secret-redaction-canary"), "{stdout}");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-redaction-canary"));
}

#[test]
fn doctor_reports_malformed_toml_without_echoing_the_line() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.toml");
    std::fs::write(&config, "[auth]\ntoken = 'secret-redaction-canary\n").unwrap();
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .args(["doctor", "--config", config.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\": \"error\""), "{stdout}");
    assert!(!stdout.contains("secret-redaction-canary"), "{stdout}");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-redaction-canary"));
}

#[test]
fn doctor_reports_a_missing_explicit_assets_directory_as_blocking() {
    let temp = tempfile::tempdir().unwrap();
    let palace = temp.path().join("untouched");
    let assets = temp.path().join("missing-assets");
    let config = temp.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[palace]\npath = {:?}\n[server]\nport = 0\n[assets]\ndir = {:?}\n",
            palace.display().to_string(),
            assets.display().to_string()
        ),
    )
    .unwrap();
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .env_remove("MEMCASTLE_ASSETS_DIR")
        .args(["doctor", "--config", config.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success(), "{stdout}");
    assert!(stdout.contains("\"check\": \"assets.dir\""), "{stdout}");
    assert!(
        stdout.contains("Required directory is missing."),
        "{stdout}"
    );
    assert!(!palace.exists());
    assert!(!assets.exists());
}

#[test]
fn doctor_offline_warns_but_succeeds_without_creating_the_palace() {
    let temp = tempfile::tempdir().unwrap();
    let palace = temp.path().join("untouched");
    let config = temp.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[palace]\npath = {:?}\n[server]\nport = 0\n",
            palace.display().to_string()
        ),
    )
    .unwrap();
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .args(["doctor", "--config", config.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\": \"warning\""), "{stdout}");
    assert!(stdout.contains("\"status\": \"skipped\""), "{stdout}");
    assert!(!palace.exists());
}

#[test]
fn mine_help_shows_the_source_first_grammar_and_no_longer_offers_the_old_flags() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["mine", "--help"])
        .assert()
        .success()
        .stdout(contains("memcastle mine <SOURCE>"))
        .stdout(contains(
            "memcastle mine opencode since=2026-09 dir=/path/to/workspace",
        ))
        .stdout(contains("memcastle mine /some/path"))
        .stdout(contains("--source").not())
        .stdout(contains("--locator").not());
}

#[test]
fn serve_help_says_it_runs_in_the_foreground() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["serve", "--help"])
        .assert()
        .success()
        .stdout(contains("foreground"));
}

#[test]
fn daemon_help_lists_start_stop_and_restart() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["daemon", "--help"])
        .assert()
        .success()
        .stdout(contains("start"))
        .stdout(contains("stop"))
        .stdout(contains("restart"));
}

#[test]
fn the_old_top_level_stop_and_restart_are_unknown_commands() {
    for word in ["stop", "restart"] {
        Command::cargo_bin("memcastle")
            .unwrap()
            .arg(word)
            .assert()
            .failure();
    }
}

#[test]
fn there_is_no_help_subcommand_because_the_flag_already_does_the_job() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .arg("help")
        .assert()
        .failure()
        .stderr(contains("unrecognized subcommand 'help'"));
}

#[test]
fn command_groups_do_not_list_a_help_entry_either() {
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .args(["job", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Matching the indented entry, not the word: `--help` itself is listed.
    assert!(!stdout.contains("\n  help "), "{stdout}");
    assert!(stdout.contains("\n  list "), "{stdout}");
}

#[test]
fn piped_help_is_plain_text_without_escape_codes() {
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .env_remove("FORCE_COLOR")
        .arg("--help")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains('\u{1b}'));
}

#[test]
fn forcing_colour_colours_help_and_no_color_wins_over_a_terminal_default() {
    let forced = Command::cargo_bin("memcastle")
        .unwrap()
        .env_remove("NO_COLOR")
        .env("FORCE_COLOR", "1")
        .arg("--help")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&forced.stdout).contains("\u{1b}["));

    let opted_out = Command::cargo_bin("memcastle")
        .unwrap()
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "1")
        .arg("--help")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&opted_out.stdout).contains('\u{1b}'));
}

#[test]
fn force_color_colours_piped_errors_unless_no_color_is_set() {
    let forced = Command::cargo_bin("memcastle")
        .unwrap()
        .env("FORCE_COLOR", "1")
        .env_remove("NO_COLOR")
        .arg("unknown-command")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&forced.stderr).contains('\u{1b}'));

    let plain = Command::cargo_bin("memcastle")
        .unwrap()
        .env("FORCE_COLOR", "1")
        .env("NO_COLOR", "1")
        .arg("unknown-command")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&plain.stderr).contains('\u{1b}'));
}

#[test]
fn completions_print_a_script_for_every_supported_shell() {
    for (shell, marker) in [
        ("bash", "_memcastle"),
        ("zsh", "#compdef memcastle"),
        ("fish", "complete -c memcastle"),
        ("powershell", "memcastle"),
        ("elvish", "memcastle"),
    ] {
        Command::cargo_bin("memcastle")
            .unwrap()
            .args(["completions", shell])
            .assert()
            .success()
            .stdout(contains(marker));
    }
}

#[test]
fn completions_offer_the_subcommands_and_the_enumerated_values() {
    let output = Command::cargo_bin("memcastle")
        .unwrap()
        .args(["completions", "fish"])
        .output()
        .unwrap();
    let script = String::from_utf8_lossy(&output.stdout);
    assert!(script.contains("completions"), "{script}");
    // `--status` and `--mode` are enumerated, so tab completion can offer them.
    assert!(script.contains("cancelled"), "--status values: {script}");
    assert!(script.contains("read_only"), "--mode values: {script}");
    assert!(
        !script.contains("-a \"help\""),
        "no help subcommand: {script}"
    );
}

#[test]
fn completions_do_not_need_a_valid_configuration() {
    // Installing tab completion must work even when the config is what is broken.
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "this is = [not valid").unwrap();
    Command::cargo_bin("memcastle")
        .unwrap()
        .arg("--config")
        .arg(&config)
        .args(["completions", "bash"])
        .assert()
        .success();
}

#[test]
fn an_unknown_shell_is_rejected_listing_the_supported_ones() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .args(["completions", "tcsh"])
        .assert()
        .failure()
        .stderr(contains("bash").and(contains("zsh")));
}

#[test]
fn a_piped_status_report_is_json_with_no_escape_codes_even_when_colour_is_forced() {
    // Colour decorates the pretty rendering, which only a terminal gets: forcing it must not
    // put escape codes into what a pipe reads. The coloured rendering itself is held by the
    // `client::status` unit tests, since a test process has no terminal to run it in.
    let (state, palace) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let forced = status_without_a_daemon(&state, &palace)
        .env("FORCE_COLOR", "1")
        .env_remove("NO_COLOR")
        .output()
        .unwrap();
    let forced = String::from_utf8_lossy(&forced.stdout);
    assert!(!forced.contains('\u{1b}'), "{forced:?}");
    serde_json::from_str::<serde_json::Value>(&forced).expect("a piped status is JSON");

    let explicit = status_without_a_daemon(&state, &palace)
        .arg("--json")
        .env("FORCE_COLOR", "1")
        .env_remove("NO_COLOR")
        .output()
        .unwrap();
    assert!(!explicit.stdout.contains(&b'\x1b'));
    serde_json::from_slice::<serde_json::Value>(&explicit.stdout)
        .expect("--json in a pipe is JSON");
}

/// A `memcastle status` with no daemon, pointed at an empty palace and a
/// state directory of its own so the developer's real registry is never read.
fn status_without_a_daemon(state: &tempfile::TempDir, palace: &tempfile::TempDir) -> Command {
    let mut command = Command::cargo_bin("memcastle").unwrap();
    command
        .env("MEMCASTLE_PALACE_PATH", palace.path())
        .env("XDG_STATE_HOME", state.path())
        .env("MEMCASTLE_BIND", "127.0.0.1")
        .env("MEMCASTLE_PORT", "1")
        .arg("status");
    command
}

#[test]
fn status_without_a_reachable_daemon_reports_not_running_and_exits_3() {
    let (state, palace) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    status_without_a_daemon(&state, &palace)
        .assert()
        // 3, not 1: a script must be able to tell "stopped" from "broken".
        .code(3)
        .stdout(contains("\"running\": false"))
        .stdout(contains("127.0.0.1:1"));
}

#[test]
fn daemon_stop_without_a_daemon_fails_and_points_at_daemon_start() {
    let (state, palace) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    Command::cargo_bin("memcastle")
        .unwrap()
        .env("MEMCASTLE_PALACE_PATH", palace.path())
        .env("XDG_STATE_HOME", state.path())
        .env("MEMCASTLE_BIND", "127.0.0.1")
        .env("MEMCASTLE_PORT", "1")
        .args(["daemon", "stop"])
        .assert()
        .failure()
        .stderr(contains("memcastle::client::not_running"))
        .stderr(contains("daemon start"));
}

#[test]
fn status_json_without_a_daemon_says_running_false_and_still_names_the_palace() {
    let (state, palace) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let output = status_without_a_daemon(&state, &palace)
        .arg("--json")
        .assert()
        .code(3)
        .get_output()
        .stdout
        .clone();

    let view: serde_json::Value = serde_json::from_slice(&output).expect("status as JSON");
    assert_eq!(view["running"], false);
    assert_eq!(view["endpoint"], "http://127.0.0.1:1");
    assert_eq!(view["endpoint_source"], "config");
    assert_eq!(view["registry"]["state"], "absent");
    assert_eq!(view["palace_path"], palace.path().display().to_string());
    assert!(view["daemon"].is_null());
}

// Liveness is only probed on Unix; elsewhere a registry file is trusted as live.
#[cfg(unix)]
#[test]
fn status_diagnoses_a_registry_file_left_behind_by_a_killed_daemon() {
    let (state, palace) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // A PID that cannot be alive: above any Linux `pid_max` (2^22).
    let dead_pid = 4_194_305_u32;
    let registry = memcastle::server::lifecycle::RuntimeInfo {
        pid: dead_pid,
        bind_addr: "127.0.0.1:1".to_string(),
        started_at: "2026-01-01T00:00:00Z".to_string(),
        version: "0.0.0".to_string(),
    };
    // Written through the same path logic the binary uses, under the
    // subprocess's `XDG_STATE_HOME`, so it lands where `status` will look.
    let file = registry_file_under(state.path(), palace.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, serde_json::to_string(&registry).unwrap()).unwrap();

    status_without_a_daemon(&state, &palace)
        .assert()
        .code(3)
        .stdout(contains("\"state\": \"stale\""))
        .stdout(contains(format!("\"pid\": {dead_pid}")));
}

#[cfg(unix)]
/// Where `memcastle` will look for `palace`'s registry when its
/// `XDG_STATE_HOME` is `state` — see `server::lifecycle::registry_path`.
fn registry_file_under(state: &std::path::Path, palace: &std::path::Path) -> std::path::PathBuf {
    let canonical = std::fs::canonicalize(palace).unwrap();
    let digest = memcastle::domain::sha256_hex(canonical.display().to_string().as_bytes());
    state
        .join("memcastle")
        .join("run")
        .join(&digest[..16])
        .join("daemon.json")
}

#[test]
fn jobs_show_rejects_a_malformed_job_id_before_ever_reaching_the_network() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .env("MEMCASTLE_BIND", "127.0.0.1")
        .env("MEMCASTLE_PORT", "1")
        .args(["job", "show", "not-a-uuid"])
        .assert()
        .failure()
        // The diagnostic code, not just a non-zero exit: a malformed id must
        // be reported as such, not as a job that "was not found".
        .stderr(contains("memcastle::jobs::id_invalid"));
}

#[test]
fn a_reserved_but_unimplemented_command_says_so_instead_of_blaming_the_config() {
    Command::cargo_bin("memcastle")
        .unwrap()
        .arg("maintenance")
        .assert()
        .failure()
        .stderr(contains("memcastle::cli::not_implemented"));
}
