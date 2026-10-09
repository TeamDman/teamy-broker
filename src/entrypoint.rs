use crate::cli::log_format::LogFormat;
use crate::failure_record::FailureRecord;
use std::io::Write;

/// Binary boundary. Failures never print an unreviewed error chain or command arguments.
/// Successful builtin requests retain Figue presentation; NDJSON parse failures use a safe record.
#[must_use]
pub fn entrypoint() -> std::process::ExitCode {
    entrypoint_with_failure_message(default_failure_message)
}

/// Allow a consumer to classify failures into reviewed static messages without printing raw errors.
#[must_use]
pub fn entrypoint_with_failure_message(
    classify: fn(&eyre::Report) -> &'static str,
) -> std::process::ExitCode {
    // This narrow scan only selects emergency encoding if normal startup cannot complete.
    let format = requested_format();
    std::panic::set_hook(Box::new(move |_| {
        let _ = report_failure(format, "Process panicked");
    }));
    match crate::main() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let message = classify(&error);
            tracing::error!(target: "teamy::failure_file", event_kind = "failure", "{message}");
            let _ = report_failure(format, message);
            std::process::ExitCode::FAILURE
        }
    }
}

pub(crate) fn requested_format() -> LogFormat {
    let args: Vec<_> = std::env::args_os()
        .skip(1)
        .take_while(|arg| arg != "--")
        .collect();
    if args
        .windows(2)
        .any(|pair| pair[0] == "--log-format" && pair[1] == "ndjson")
        || args.iter().any(|arg| arg == "--log-format=ndjson")
    {
        LogFormat::Ndjson
    } else {
        LogFormat::Text
    }
}

fn report_failure(format: LogFormat, message: &str) -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    match format {
        LogFormat::Text => writeln!(stderr, "ERROR: {message}"),
        LogFormat::Ndjson => {
            let record = FailureRecord {
                level: "ERROR",
                event_kind: "failure",
                message,
            };
            let encoded = facet_json::to_string(&record).map_err(std::io::Error::other)?;
            writeln!(stderr, "{encoded}")
        }
    }
}

pub(crate) fn finish_parse(
    outcome: figue::DriverOutcome<crate::cli::Cli>,
) -> eyre::Result<crate::cli::Cli> {
    match outcome.into_result() {
        Ok(output) => Ok(output.value),
        Err(error) => {
            // Parse diagnostics can contain the entire argv, including mail bodies or secrets.
            if !error.is_success() {
                return Err(eyre::eyre!("invalid command line"));
            }
            Ok(figue::DriverOutcome::<crate::cli::Cli>::err(error).unwrap())
        }
    }
}

pub(crate) fn default_failure_message(error: &eyre::Report) -> &'static str {
    if error.to_string().contains("Re-run with --force") {
        "Destination contains generated files; use --force only for intentional overwrite"
    } else if error.to_string() == "invalid command line" {
        "Invalid command line; use --help"
    } else {
        "Command failed; check configuration and diagnostic events"
    }
}
