//! The shared CLI process path writes span timings while using an owned status fixture.

use std::env;
use std::fs;
use std::process;
use teamy_broker::protocol::Status;
#[path = "support/logging_cli.rs"]
mod logging_cli;

use std::time::SystemTime;

#[test]
fn command_span_timings_reach_ndjson_without_cluttering_stdio() -> eyre::Result<()> {
    let run_id = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_nanos();
    let directory =
        env::temp_dir().join(format!("teamy-broker-span-test-{}-{run_id}", process::id()));
    fs::create_dir_all(&directory)?;
    let log_path = directory.join("timings.ndjson");
    let output = logging_cli::command()
        .args(["--log-filter", "info", "--log-file"])
        .arg(&log_path)
        .args(["--output-format", "json", "status"])
        .output()?;
    let ndjson = fs::read_to_string(&log_path)?;
    fs::remove_dir_all(&directory)?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    let status: Status = facet_json::from_str(&stdout)?;
    assert!(status.running, "isolated status fixture must answer");
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
