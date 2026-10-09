use facet::Facet;
use std::fs;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Debug, Facet)]
struct LogLine {
    level: String,
}

fn scratch() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "teamy-log-test-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[test]
fn independent_defaults_preserve_debug_in_file_and_numeric_progress_on_stderr() {
    let root = scratch();
    let file = root.join("capture.ndjson");
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
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
    assert!(stderr.contains("\"completed\":0"));
    assert!(stderr.contains("\"total\":1"));
    assert!(stderr.contains("\"state\":\"completed\""));
    let file_text = fs::read_to_string(&file).unwrap();
    assert!(
        file_text
            .lines()
            .map(|line| facet_json::from_str::<LogLine>(line).unwrap())
            .any(|record| record.level == "DEBUG")
    );
    assert!(!file_text.contains('\u{1b}'));
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("Task progress")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stderr_off_does_not_disable_file_and_existing_files_are_appended() {
    let root = scratch();
    fs::create_dir_all(&root).unwrap();
    let file = root.join("capture.log");
    fs::write(&file, "prior record\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
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
