#[cfg(not(windows))]
compile_error!("teamy-broker V1 supports Windows only");
mod entrypoint;
mod failure_record;
pub use entrypoint::entrypoint;
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
    // Install color_eyre for better error reports
    // The binary owns emergency error/panic encoding; library callers own their hooks.
    let cancellation_token = CtrlCHandler::default().install()?;

    // Parse command line arguments using figue
    // unwrap() is figue's intended CLI entry behavior:
    // it exits with proper codes for --help/--version/completions/parse-errors.
    let version = version();

    let cli: figue::DriverOutcome<Cli> = figue::Driver::new(
        figue::builder::<Cli>()
            .expect("schema should be valid")
            .cli(move |cli| cli.args_os(std::env::args_os().skip(1)).strict())
            .help(move |help| {
                // TODO(template): replace the implementation git URL with the generated repository path.
                help.version(version)
                    .include_implementation_source_file(true)
                    .include_implementation_github_url(
                        "TeamDman/teamy-broker",
                        env!("GIT_REVISION"),
                    )
            })
            .build(),
    )
    .run();
    let cli = entrypoint::finish_parse(cli)?;

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
        let started = std::time::Instant::now();

        // Invoke whatever command was requested and render its output once at the top level
        let requested_output_format = cli.global_args.output_format;
        let output = cli.invoke(cancellation_token.clone())?;
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
