//! The actual CLI writes span timing receipts to its log file, separately from stdout.

use std::process::Command;
use std::time::SystemTime;

#[test]
fn command_span_timings_reach_ndjson_without_cluttering_stdio() -> eyre::Result<()> {
    let run_id = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "teamy-broker-span-test-{}-{run_id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory)?;
    let log_path = directory.join("timings.ndjson");
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .args(["--log-filter", "info", "--log-file"])
        .arg(&log_path)
        .args(["--output-format", "json", "status"])
        .output()?;
    let ndjson = std::fs::read_to_string(&log_path)?;
    std::fs::remove_dir_all(&directory)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("\"running\""), "{stdout}");
    assert!(!stdout.contains("time.busy"), "{stdout}");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(!stderr.contains("time.busy"), "{stderr}");
    let closes: Vec<_> = ndjson
        .lines()
        .filter(|line| line.contains("\"message\":\"close\""))
        .collect();
    assert_eq!(closes.len(), 1, "{ndjson}");
    assert!(closes[0].contains("\"name\":\"command\""), "{ndjson}");
    assert!(closes[0].contains("\"time.busy\""), "{ndjson}");
    assert!(closes[0].contains("\"time.idle\""), "{ndjson}");
    Ok(())
}
