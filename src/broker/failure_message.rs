/// Classify into reviewed static messages. Error payloads are never diagnostic output.
#[must_use]
pub fn classify(error: &eyre::Report) -> &'static str {
    match error.to_string().as_str() {
        "invalid command line" => "Invalid command line; use --help",
        "select exactly one of --all or --lease-id" => "Select exactly one of --all or --lease-id",
        "broker unavailable" => "Broker is not running; start serve explicitly",
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
