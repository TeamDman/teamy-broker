/// Classify into reviewed static messages. Error payloads are never diagnostic output.
#[must_use]
pub fn classify(error: &eyre::Report) -> &'static str {
    if error.downcast_ref::<crate::protocol::FailureKind>()
        == Some(&crate::protocol::FailureKind::ShutdownBlocked)
    {
        return crate::protocol::FailureKind::ShutdownBlocked.message();
    }
    match error.to_string().as_str() {
        "invalid command line" => "Invalid command line; use --help",
        "select exactly one of --all or --lease-id" => "Select exactly one of --all or --lease-id",
        "broker shutdown status unsupported; update broker explicitly" => {
            "Broker lacks guarded shutdown status; update it explicitly before stopping"
        }
        "broker unavailable" => "Broker is not running; start serve explicitly",
        "broker access denied" => {
            "Access to the broker endpoint was denied; use an authorized execution context without weakening its ACL"
        }
        "broker control failed" => {
            "Broker refused the operation or cache cleanup failed; inspect redacted status"
        }
        "broker shutdown deadline exceeded" => "Broker did not finish shutdown before the deadline",
        "cache cleanup incomplete" => {
            "Lease access was removed, but persistent cleanup is incomplete"
        }
        "incompatible broker; refusing autostart" => {
            "Existing broker is incompatible; update it explicitly"
        }
        _ => "Command failed; check configuration and diagnostic events",
    }
}
