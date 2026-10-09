/// Resolve exactly one explicit consumer provider. Even equal CLI and environment values conflict.
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn resolve_provider(
    cli: Option<&str>,
    environment: Option<&str>,
) -> eyre::Result<&'static str> {
    let selected = match (cli, environment) {
        (Some(_), Some(_)) => eyre::bail!("CLI and environment both select a provider"),
        (None, None) => eyre::bail!("an explicit provider is required"),
        (Some(value), None) | (None, Some(value)) => value,
    };
    match selected {
        "teamy-broker" => Ok("teamy-broker"),
        "windows-credential-manager" => Ok("windows-credential-manager"),
        "well-known-file" => Ok("well-known-file"),
        _ => eyre::bail!("invalid explicit provider"),
    }
}
