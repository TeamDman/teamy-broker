use super::Command;
use super::output::OutputFormat;

pub(crate) const UNSUPPORTED_CSV: &str =
    "CSV is not supported for this command; use --output-format json or text";

/// Reject unsupported result shapes before logging, input collection or dispatch.
pub(crate) fn validate(command: &Command, format: Option<OutputFormat>) -> eyre::Result<()> {
    if format != Some(OutputFormat::Csv) {
        return Ok(());
    }
    let supported = match command {
        Command::Serve(_)
        | Command::Cleanup(_)
        | Command::Purge(_)
        | Command::Stop(_)
        | Command::PhonePairCancel(_)
        | Command::PhonePairConfirm(_)
        | Command::PhonePairInspect(_) => true,
        Command::Preview(args) => !args.show_test_dialog,
        Command::Status(_)
        | Command::PhonePair(_)
        | Command::PhoneRevoke(_)
        | Command::PhoneList(_)
        | Command::PhoneStatus(_)
        | Command::PhoneAuthorize(_)
        | Command::PhoneConfigure(_)
        | Command::PhoneConfigShow(_)
        | Command::PhoneConfigValidate(_) => false,
    };
    eyre::ensure!(supported, UNSUPPORTED_CSV);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::cli::output::CliOutput;
    use crate::protocol::pairing_comparison::PairingConfirmationResult;
    use crate::protocol::pairing_comparison::PairingInspection;

    fn command(args: &[&str]) -> Command {
        figue::Driver::new(
            figue::builder::<Cli>()
                .unwrap()
                .cli(|cli| cli.args(args.iter().copied()).strict())
                .build(),
        )
        .run()
        .into_result()
        .unwrap()
        .value
        .command
    }

    #[test]
    fn csv_retains_flat_and_absent_results() {
        for args in [
            ["serve"].as_slice(),
            ["stop"].as_slice(),
            ["purge", "--all"].as_slice(),
            ["cleanup", "--lease-id", "fixture-id"].as_slice(),
            ["preview"].as_slice(),
            ["phone-pair-inspect", "--attempt-id", "fixture-id"].as_slice(),
            ["phone-pair-cancel", "--attempt-id", "fixture-id"].as_slice(),
            [
                "phone-pair-confirm",
                "--attempt-id",
                "fixture-id",
                "--comparison-stdin",
            ]
            .as_slice(),
        ] {
            validate(&command(args), Some(OutputFormat::Csv)).unwrap();
        }

        let mut bytes = Vec::new();
        CliOutput::facet(
            PairingConfirmationResult {
                attempt_id: "fixture".into(),
                accepted: true,
            },
            None,
        )
        .emit_to(&mut bytes, Some(OutputFormat::Csv), false)
        .unwrap();
        assert_eq!(bytes, b"fixture,true\n");
        bytes.clear();
        CliOutput::facet(
            PairingInspection {
                attempt_id: "fixture".into(),
                expires_unix_ms: 1,
                comparison: "fixture-comparison".into(),
                remaining_wrong_attempts: 2,
            },
            None,
        )
        .emit_to(&mut bytes, Some(OutputFormat::Csv), false)
        .unwrap();
        assert_eq!(bytes, b"fixture,1,fixture-comparison,2\n");
    }

    #[test]
    fn nested_results_require_text_or_json() {
        for args in [
            ["status"].as_slice(),
            ["phone-pair"].as_slice(),
            ["phone-revoke"].as_slice(),
            ["phone-list"].as_slice(),
            ["phone-status"].as_slice(),
            ["preview", "--show-test-dialog"].as_slice(),
            ["phone-config-show"].as_slice(),
            ["phone-config-validate"].as_slice(),
        ] {
            let command = command(args);
            assert_eq!(
                validate(&command, Some(OutputFormat::Csv))
                    .unwrap_err()
                    .to_string(),
                UNSUPPORTED_CSV
            );
            for format in [None, Some(OutputFormat::Text), Some(OutputFormat::Json)] {
                validate(&command, format).unwrap();
            }
        }
    }
}
