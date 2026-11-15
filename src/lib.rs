use std::env;
use std::ffi::OsString;
use std::time::Instant;
#[cfg(not(windows))]
compile_error!("teamy-broker V1 supports Windows only");
mod entrypoint;
mod failure_record;
pub use entrypoint::entrypoint;
pub use entrypoint::entrypoint_with_client;
pub use entrypoint::entrypoint_with_failure_message;
pub mod cli;
pub mod elapsed_duration;
pub mod logging_init;
pub mod paths;
#[cfg(windows)]
mod windows_startup;

use crate::cli::Cli;
use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use std::io::IsTerminal;
use std::io::{self};
use teamy_cancellation::CtrlCHandler;

/// Version string combining package version, git repository metadata, and build time.
fn version() -> String {
    let built_at = option_env!("BUILD_TIMESTAMP_UNIX")
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .map_or_else(
            || "unknown build time".to_string(),
            |timestamp| {
                timestamp
                    .with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S %Z")
                    .to_string()
            },
        );

    format!(
        "{} (repo {}, branch {}, rev {}, worktree {}, built {})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_REPOSITORY_URL"),
        env!("GIT_BRANCH"),
        env!("GIT_REVISION"),
        env!("GIT_WORKTREE_STATUS"),
        built_at,
    )
}

/// Entrypoint for the program.
///
/// # Errors
///
/// Returns startup, logging, command execution or output errors. The binary owns safe diagnostic presentation.
///
/// # Panics
///
/// Panics if the CLI schema is invalid (should never happen with correct code).
pub fn main() -> eyre::Result<()> {
    main_with_client(None)
}

pub(crate) fn main_with_client(client: Option<sdk::Client>) -> eyre::Result<()> {
    run_cli(parse_cli()?, client)
}

pub(crate) fn parse_cli() -> eyre::Result<Cli> {
    entrypoint::finish_parse(parse_arguments(env::args_os().skip(1))?)
}

fn parse_arguments(
    args: impl IntoIterator<Item = OsString>,
) -> eyre::Result<figue::DriverOutcome<Cli>> {
    // This Figue revision silently discards non-Unicode args_os values. Reject
    // them before parsing so an invalid flag cannot disappear from a command.
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_redacted_arg| eyre::eyre!("invalid command line"))
        })
        .collect::<eyre::Result<_>>()?;
    let version = version();
    Ok(figue::Driver::new(
        figue::builder::<Cli>()
            .expect("schema should be valid")
            .cli(move |cli| cli.args(args).strict())
            .help(move |help| {
                help.version(version)
                    .include_implementation_source_file(true)
                    .include_implementation_github_url(
                        "TeamDman/teamy-broker",
                        env!("GIT_REVISION"),
                    )
            })
            .build(),
    )
    .run())
}

pub(crate) fn run_cli(cli: Cli, client: Option<sdk::Client>) -> eyre::Result<()> {
    // Cancellation diagnostics belong to the configured tracing sinks. The upstream
    // default writes a raw ANSI marker, including when stderr is NDJSON or disabled.
    let cancellation_token = CtrlCHandler {
        should_eprintln_on_ctrl_c: false,
        ..CtrlCHandler::default()
    }
    .install()?;

    let _stop_after_duration_thread = cli
        .global_args
        .stop_after
        .start_stop_after_duration_thread(cancellation_token.clone())?;

    // Initialize logging
    logging_init::init_logging(&cli.global_args, cancellation_token.clone())?;

    #[cfg(windows)]
    {
        // Enable ANSI support on Windows
        // This fails in a pipe scenario, so we ignore the error
        let _ = windows_startup::enable_ansi_support();

        // Warn if UTF-8 is not enabled on Windows
        #[cfg(windows)]
        windows_startup::warn_if_utf8_not_enabled();
    };

    let command_name = cli.command.name();
    tracing::debug!(command = command_name, "Dispatching command");
    let result = (|| -> eyre::Result<()> {
        // Measure command work after logging setup, including output and early returns.
        let command_name = cli.command.name();
        let _command_span = tracing::info_span!("command", command = command_name).entered();
        let started = Instant::now();

        // Invoke whatever command was requested and render its output once at the top level
        let requested_output_format = cli.global_args.output_format;
        let output = cli.invoke_with_client(cancellation_token.clone(), client)?;
        let threshold = output.elapsed_duration_warn_threshold;
        cancellation_token.bail_if_cancelled()?;
        let stdout = io::stdout();
        let output_is_terminal = stdout.is_terminal();
        let emitted = output.emit_to(
            &mut stdout.lock(),
            requested_output_format,
            output_is_terminal,
        );
        elapsed_duration::warn_if_exceeded(command_name, started.elapsed(), threshold);
        emitted?;
        cancellation_token.bail_if_cancelled()?;
        Ok(())
    })();
    tracing::debug!(
        command = command_name,
        success = result.is_ok(),
        "Command finished"
    );
    result
}

#[cfg(windows)]
pub mod broker;
#[cfg(windows)]
pub mod platform;
pub mod protocol;
#[cfg(windows)]
pub mod sdk;

#[cfg(test)]
extern crate self as teamy_broker;

#[cfg(test)]
mod startup_contract_tests {
    use super::parse_arguments;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    #[test]
    fn invalid_unicode_arguments_are_rejected_instead_of_discarded() {
        let args = [OsString::from("status"), OsString::from_wide(&[0xd800])];
        let error = parse_arguments(args).expect_err("invalid argv must fail");
        assert_eq!(error.to_string(), "invalid command line");
    }

    #[test]
    fn parsing_is_repeatable_without_process_startup() {
        for args in [["--help"].as_slice(), ["--version"].as_slice()] {
            for _ in 0..2 {
                let outcome = parse_arguments(args.iter().copied().map(OsString::from)).unwrap();
                let builtin = outcome.into_result().expect_err("builtin response");
                assert!(builtin.is_success());
            }
        }
    }
}
