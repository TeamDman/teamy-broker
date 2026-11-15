use std::env;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use teamy_broker::protocol::Status;
#[path = "support/logging_cli.rs"]
mod logging_cli;

use facet::Facet;
use std::fs;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Debug, Facet)]
struct LogLine {
    level: String,
}

fn scratch() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    env::temp_dir().join(format!(
        "teamy-log-test-{}-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn independent_defaults_keep_command_lifecycle_in_debug_file() {
    let root = scratch();
    let file = root.join("capture.ndjson");
    let output = logging_cli::command()
        .args([
            "--log-format",
            "ndjson",
            "--output-format",
            "json",
            "--log-file",
        ])
        .arg(&file)
        .args(["status"])
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    for line in stderr.lines() {
        let record: LogLine = facet_json::from_str(line).unwrap();
        assert_ne!(record.level, "DEBUG");
    }
    assert!(!stderr.contains("\"task_id\":\"command\""));
    let file_text = fs::read_to_string(&file).unwrap();
    assert!(
        file_text
            .lines()
            .map(|line| facet_json::from_str::<LogLine>(line).unwrap())
            .any(|record| record.level == "DEBUG")
    );
    assert!(file_text.contains("Dispatching command"));
    assert!(file_text.contains("Command finished"));
    assert!(!file_text.contains("progress_event.rs"));
    let dispatch = file_text
        .lines()
        .find(|line| line.contains("Dispatching command"))
        .unwrap();
    assert!(dispatch.contains("lib.rs"));
    assert!(dispatch.contains("\"level\":\"DEBUG\""));
    assert!(!dispatch.contains("origin_file"));
    assert!(!stderr.contains("Dispatching command"));
    assert!(!stderr.contains("Command finished"));
    assert!(!file_text.contains('\u{1b}'));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("Task progress"));
    let status: Status = facet_json::from_str(&stdout).unwrap();
    assert!(status.running, "isolated status fixture must answer");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stderr_off_does_not_disable_file_and_existing_files_are_appended() {
    let root = scratch();
    fs::create_dir_all(&root).unwrap();
    let file = root.join("capture.log");
    fs::write(&file, "prior record\n").unwrap();
    let output = logging_cli::command()
        .args([
            "--log-filter",
            "off",
            "--log-file-format",
            "text",
            "--log-file",
        ])
        .arg(&file)
        .args(["status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let status: Status = facet_json::from_str(&stdout).unwrap();
    assert!(status.running, "isolated status fixture must answer");
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("prior record\n"));
    assert!(text.contains("Tracing initialized"));
    assert!(!text.contains('\u{1b}'));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_filters_have_no_file_effects_and_emergency_json_omits_raw_payloads() {
    let root = scratch();
    let file = root.join("SYNTHETIC_SECRET_NEVER_LOG.ndjson");
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .args([
            "--log-format",
            "ndjson",
            "--log-file-filter",
            "fixture=bogus",
            "--log-file",
        ])
        .arg(&file)
        .args(["status"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!file.exists());
    let stderr = String::from_utf8(output.stderr).unwrap();
    for line in stderr.lines() {
        let _: LogLine = facet_json::from_str(line).unwrap();
    }
    assert!(!stderr.contains("SYNTHETIC_SECRET"));
}

#[test]
fn ndjson_parse_failures_are_safe_and_help_does_not_open_a_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .args(["--log-format", "ndjson", "--unknown-SYNTHETIC_SECRET"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("SYNTHETIC_SECRET"));
    for line in stderr.lines() {
        let _: LogLine = facet_json::from_str(line).unwrap();
    }
    let root = scratch();
    let file = root.join("must-not-exist.ndjson");
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .args(["--log-format", "ndjson", "--log-file"])
        .arg(&file)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!file.exists());
}

#[test]
fn text_parse_failure_cannot_echo_sensitive_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .args(["--unknown-SYNTHETIC_SECRET"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("SYNTHETIC_SECRET")
    );
}

#[test]
fn builtins_do_not_validate_filters_or_create_log_directories() {
    let root = scratch();
    let file = root.join("must-not-exist").join("capture.ndjson");
    for builtin in [
        ["--help"].as_slice(),
        ["--version"].as_slice(),
        ["phone-pair-confirm", "--help"].as_slice(),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
            .args([
                "--log-format",
                "ndjson",
                "--log-filter",
                "fixture=bogus",
                "--log-file",
            ])
            .arg(&file)
            .args(builtin)
            .env("RUST_LOG", "fixture=bogus")
            .output()
            .unwrap();
        assert!(output.status.success(), "builtin failed: {builtin:?}");
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty(), "builtins must not start logging");
        assert!(!root.exists(), "builtins must not create log directories");
    }
}
