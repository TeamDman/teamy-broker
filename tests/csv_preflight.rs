use std::env;
use teamy_broker::cli::Cli;
use teamy_cancellation::CancellationToken;

#[path = "support/logging_cli.rs"]
mod logging_cli;

#[test]
fn unsupported_csv_fails_before_log_file_creation() {
    let root = env::temp_dir().join(format!("teamy-csv-preflight-{}", uuid::Uuid::new_v4()));
    let file = root.join("must-not-exist.ndjson");
    // The injected client owns an isolated status-only pipe even if preflight regresses.
    let output = logging_cli::command()
        .args([
            "--output-format",
            "csv",
            "--log-format",
            "ndjson",
            "--log-file",
        ])
        .arg(&file)
        .arg("status")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        !root.exists(),
        "format rejection must precede logging effects"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("use --output-format json or text"));
    assert_eq!(stderr.lines().count(), 1, "no command lifecycle may begin");
    assert!(!stderr.contains('\u{1b}'));
}

#[test]
fn direct_cli_invocation_applies_format_preflight_before_cancellation_or_dispatch() {
    let cli = figue::Driver::new(
        figue::builder::<Cli>()
            .unwrap()
            .cli(|cli| cli.args(["--output-format", "csv", "status"]).strict())
            .build(),
    )
    .run()
    .into_result()
    .unwrap()
    .value;
    let cancellation = CancellationToken::new();
    cancellation.request_cancel("fixture must not dispatch");
    let error = cli.invoke(cancellation).unwrap_err();
    assert_eq!(
        error.to_string(),
        "CSV is not supported for this command; use --output-format json or text"
    );
}
