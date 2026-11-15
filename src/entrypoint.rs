use crate::cli::Cli;
use crate::cli::log_format::LogFormat;
use crate::cli::output_policy::UNSUPPORTED_CSV;
use crate::failure_record::FailureRecord;
use crate::sdk::Client;
use std::env;
use std::io;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::io::Write;
use std::panic;
use std::process::ExitCode;

/// Binary boundary. Failures never print an unreviewed error chain or command arguments.
/// Successful builtin requests retain Figue presentation; NDJSON parse failures use a safe record.
#[must_use]
pub fn entrypoint() -> ExitCode {
    entrypoint_with_failure_message(default_failure_message)
}

/// Allow a consumer to classify failures into reviewed static messages without printing raw errors.
#[must_use]
pub fn entrypoint_with_failure_message(classify: fn(&eyre::Report) -> &'static str) -> ExitCode {
    run(None, classify)
}

/// Run the same CLI process boundary with an explicitly supplied broker client.
/// The ordinary binary continues to resolve the current user's endpoint lazily.
#[must_use]
pub fn entrypoint_with_client(
    client: Client,
    classify: fn(&eyre::Report) -> &'static str,
) -> ExitCode {
    run(Some(client), classify)
}

fn run(client: Option<Client>, classify: fn(&eyre::Report) -> &'static str) -> ExitCode {
    // This narrow scan only selects emergency encoding if normal startup cannot complete.
    let format = requested_format();
    let result = crate::parse_cli().and_then(|cli| {
        panic::set_hook(Box::new(move |_| {
            let _ = report_failure(format, "Process panicked");
        }));
        crate::run_cli(cli, client)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let message = if error.to_string() == UNSUPPORTED_CSV {
                UNSUPPORTED_CSV
            } else {
                classify(&error)
            };
            tracing::error!(target: "teamy::failure_file", event_kind = "failure", "{message}");
            let _ = report_failure(format, message);
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn requested_format() -> LogFormat {
    let args: Vec<_> = env::args_os()
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

fn report_failure(format: LogFormat, message: &str) -> IoResult<()> {
    let mut stderr = io::stderr().lock();
    match format {
        LogFormat::Text => writeln!(stderr, "ERROR: {message}"),
        LogFormat::Ndjson => {
            let record = FailureRecord {
                level: "ERROR",
                event_kind: "failure",
                message,
            };
            let encoded = facet_json::to_string(&record).map_err(IoError::other)?;
            writeln!(stderr, "{encoded}")
        }
    }
}

pub(crate) fn finish_parse(outcome: figue::DriverOutcome<Cli>) -> eyre::Result<Cli> {
    match outcome.into_result() {
        Ok(output) => Ok(output.value),
        Err(error) => {
            // Parse diagnostics can contain the entire argv, including mail bodies or secrets.
            if !error.is_success() {
                return Err(eyre::eyre!("invalid command line"));
            }
            Ok(figue::DriverOutcome::<Cli>::err(error).unwrap())
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
